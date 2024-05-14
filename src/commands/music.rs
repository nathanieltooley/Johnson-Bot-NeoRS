use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    async_trait, ChannelId, Color, CreateEmbed, CreateMessage, GuildId, Http,
};
use songbird::input::{AuxMetadata, Compose, YoutubeDl};
use songbird::tracks::{PlayMode, Track};
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler, TrackEvent};
use tokio::sync::{Mutex, MutexGuard};
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;
use crate::spotify::{self, get_tracks_from_url};

static DRIVER_EVENTS_ADDED: AtomicBool = AtomicBool::new(false);

// I know static stuff is frowned apon, but like cmon
// this is the best way of doing this, mainly because
// the event has to take ownership of the handler
//
// Mutex is added here for interior mutability
static TRACK_METADATA_MAP: Lazy<Mutex<HashMap<Uuid, Arc<TrackMetadata>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static SONG_MESSAGE_COLOR_MAP: Lazy<HashMap<&str, Color>> = Lazy::new(|| {
    let mut m = HashMap::new();

    m.insert("youtube.com", Color::RED);
    m.insert("youtu.be", Color::RED);
    m.insert("open.spotify.com", Color::DARK_GREEN);

    m
});

struct TrackMetadata {
    aux_meta: AuxMetadata,
    url: Url,
}

struct DriverReconnectHandler;
struct DriverDisconnectHandler;

struct TrackEventHandler {
    track_meta: Arc<TrackMetadata>,
}
struct TrackErrorHandler {
    track_meta: Arc<TrackMetadata>,
}
struct TrackNotifier {
    channel_id: ChannelId,
    http_handle: Arc<Http>,
}

#[async_trait]
impl EventHandler for DriverReconnectHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::DriverReconnect(cd) = ctx {
            info!(
                "Voice Driver reconnected to Channel ID: {}, Guild ID: {}",
                cd.channel_id
                    .map_or(String::from("No Channel"), |cid| cid.to_string()),
                cd.guild_id,
            );
        };

        None
    }
}

#[async_trait]
impl EventHandler for DriverDisconnectHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::DriverDisconnect(cd) = ctx {
            info!(
                "Voice Driver disconnected from Channel ID: {}, Guild ID: {}",
                cd.channel_id
                    .map_or(String::from("No Channel"), |cid| cid.to_string()),
                cd.guild_id,
            );
        }

        None
    }
}

#[async_trait]
impl EventHandler for TrackEventHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (t_state, t_handle) in *track_list {
                // &Option<String> -> Option<&String>
                let title = self
                    .track_meta
                    .aux_meta
                    .title
                    .as_deref()
                    .unwrap_or("No Title");

                let artist = self
                    .track_meta
                    .aux_meta
                    .artist
                    .as_deref()
                    .unwrap_or("No Artist");

                info!(
                    "Track {} : {} updated to state: {:?}",
                    title, artist, t_state
                );

                if t_state.playing == PlayMode::End {
                    match TRACK_METADATA_MAP
                        .lock()
                        .await
                        .remove_entry(&t_handle.uuid())
                    {
                        Some(_) => {
                            debug!("Deleted metadata from UUID: {}", t_handle.uuid());
                        }
                        None => {
                            warn!("Track metadata cleanup attempted to remove metadata using a handle that was not in the map");
                        }
                    }
                }
            }
        }

        None
    }
}

#[async_trait]
impl EventHandler for TrackErrorHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (t_state, _t_handle) in *track_list {
                let title = self
                    .track_meta
                    .aux_meta
                    .title
                    .as_deref()
                    .unwrap_or("No Title");
                let artist = self
                    .track_meta
                    .aux_meta
                    .artist
                    .as_deref()
                    .unwrap_or("No Artist");

                info!(
                    "Track {} : {} Encountered an error: {:?}",
                    title, artist, t_state.playing
                )
            }
        }

        None
    }
}

#[async_trait]
impl EventHandler for TrackNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (_t_state, t_handle) in *track_list {
                let track_message_result = self
                    .channel_id
                    .send_message(
                        &self.http_handle,
                        CreateMessage::new().embed(create_song_embed(
                            TRACK_METADATA_MAP
                                .lock()
                                .await
                                .get(&t_handle.uuid())
                                .expect(
                                    "track metadata should've been added to the track notifier map",
                                ),
                        )),
                    )
                    .await;

                if let Err(e) = track_message_result {
                    error!("Could not send track play notification message: {e:?}");
                }
            }
        }

        None
    }
}

/// Will join the given channel in the given Guild either by creating a new call object and joining
/// the channel or by utilizing an already existing call object and switching channels
async fn join(
    ctx: &Context<'_>,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<Arc<Mutex<Call>>, Error> {
    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird should be registered with Johnson Bot")
        .clone();

    match manager.get(guild_id) {
        Some(call) => {
            debug!("Call object already created, attempting to connect again to channel");
            let mut call_handle = call.lock().await;

            debug!("Attempting to join channel with call object");
            match call_handle.current_channel() {
                // If we have established a "Call" object but we're not in a channel
                // Join this one
                //
                // We have to use the join method on the call handle
                // as the manager join method will not work

                // This matches the None case further down
                None => match call_handle.join(channel_id).await {
                    Ok(_) => {
                        debug!("Johnson joined a voice channel!");

                        // We have to drop the MutexGuard because it is borrowing call
                        drop(call_handle);
                        Ok(call)
                    }
                    Err(e) => {
                        error!("Johnson failed to join a voice channel! {e:?}");
                        Err(Box::new(e))
                    }
                },
                Some(_) => {
                    debug!("Johnson already in channel, no need to reconnect");
                    drop(call_handle);
                    Ok(call)
                }
            }
        }
        None => {
            debug!("Call has not been created, creating now");
            // If the bot is not in a call, join the user's channel
            match manager.join(guild_id, channel_id).await {
                Ok(call) => {
                    debug!("Johnson joined a voice channel");
                    Ok(call)
                }
                Err(e) => {
                    debug!("Johnson failed to join a voice channel {e}");
                    // Return join error
                    Err(Box::new(e))
                }
            }
        }
    }
}

async fn attach_event_handlers(voice_lock: &mut MutexGuard<'_, Call>) {
    voice_lock.add_global_event(CoreEvent::DriverDisconnect.into(), DriverDisconnectHandler);
    voice_lock.add_global_event(CoreEvent::DriverReconnect.into(), DriverReconnectHandler);
}

fn create_song_embed(metadata: &TrackMetadata) -> CreateEmbed {
    let color = *SONG_MESSAGE_COLOR_MAP
        .get(
            metadata
                .url
                .host_str()
                .expect("should have valid domain to be considered for playback"),
        )
        .unwrap_or(&Color::GOLD);

    let embed = CreateEmbed::new()
        .title("NOW PLAYING")
        .field(
            "Song Name: ",
            metadata.aux_meta.title.as_deref().unwrap_or("No Name"),
            false,
        )
        .field(
            "Song Artist: ",
            metadata.aux_meta.artist.as_deref().unwrap_or("No Artist"),
            false,
        )
        .color(color);

    // If the metadata contains a thumbnail, create an embed thumbnail
    let embed = match metadata.aux_meta.thumbnail.as_deref() {
        Some(thumbnail) => embed.image(thumbnail),
        None => embed,
    };

    embed
}

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn play(ctx: Context<'_>, url: String) -> Result<(), Error> {
    if ctx.guild().is_none() {
        ctx.reply("Cannot run this command outside of a guild")
            .await?;
        return Ok(());
    }

    ctx.defer().await?;

    let parsed_url = match Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            error!("Could not parse URL: {e}");
            ctx.say(format!("Could not parse URL: {e}")).await?;
            return Ok(());
        }
    };

    debug!("{parsed_url:?}");

    let (guild_id, channel_id) = {
        let guild = ctx.guild().unwrap();

        // Get the VC the user is connected to
        let channel_id = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id);

        (guild.id, channel_id)
    };

    // Have to do this check outside of the above block because of weird stuff
    // with async and guild
    let channel_id = match channel_id {
        Some(c) => c,
        None => {
            ctx.reply("Cannot use command outside of voice channel")
                .await?;
            return Ok(());
        }
    };

    let mut ytdl_tracks: Vec<YoutubeDl> = Vec::new();

    match parsed_url.host() {
        Some(h) => {
            match h.to_string().as_str() {
                "youtube.com" | "youtu.be" => {
                    debug!("User passed in Youtube link");

                    let http_client = ctx.data().http.clone();

                    ytdl_tracks.push(YoutubeDl::new(http_client, url));
                }
                "open.spotify.com" => {
                    let spotify = &ctx.data().spotify_client;
                    let http_client = &ctx.data().http;
                    let tracks = get_tracks_from_url(spotify, &parsed_url).await?;

                    debug!("Enqueuing {} tracks from spotify", tracks.len());

                    ctx.say(format!("Enqueuing {} tracks from Spotify", tracks.len())).await?;

                    ytdl_tracks.append(&mut tracks.iter().map(|track| {
                        // Vec<Artists> -> Vec<&String> -> Vec<&str> -> String
                        let artists = track.artists.iter().map(|artist| &artist.name).map(String::as_str).collect::<Vec<_>>().join(" ");
                        let mut search_string = String::new();
                        search_string.push_str(&format!("{} {}", track.name, artists));

                        YoutubeDl::new_search(http_client.clone(), search_string)
                    }).collect())
                }
                _ => {
                    ctx.say("Invalid URL").await?;
                    return Ok(());
                }
            }
        }
        None => {
            ctx.say("Invalid URL").await?;
            return Ok(());
        }
    }

    debug!("Attempting to join VC");

    let vc = join(&ctx, guild_id, channel_id).await?;

    {
        let mut h_lock = vc.lock().await;

        if !DRIVER_EVENTS_ADDED.load(Relaxed) {
            debug!("Attaching event handlers to global driver");

            attach_event_handlers(&mut h_lock).await;
            let track_notif = TrackNotifier {
                http_handle: ctx.serenity_context().http.clone(),
                channel_id: ctx.channel_id(),
            };

            h_lock.add_global_event(Event::Track(TrackEvent::Play), track_notif);

            DRIVER_EVENTS_ADDED.swap(true, Relaxed);
        }

        let tracks_len = ytdl_tracks.len();

        for mut ytdl in ytdl_tracks {
            let meta = ytdl.aux_metadata().await?;
            let track_meta = Arc::new(TrackMetadata {
                aux_meta: meta,
                url: parsed_url.clone(),
            });

            let title = &track_meta.aux_meta.title.as_deref().unwrap_or("No Title");
            let author = &track_meta.aux_meta.artist.as_deref().unwrap_or("No Author");

            // Create track here so we can get the UUID
            // and insert it into the map before it gets played
            let track = Track::new(ytdl.into());

            TRACK_METADATA_MAP
                .lock()
                .await
                .insert(track.uuid, track_meta.clone());

            let t_handle = h_lock.enqueue(track).await;

            if tracks_len == 1 {
                ctx.say(format!("Enqueuing `{}`, by `{}`", title, author))
                    .await?;
            }

            t_handle.add_event(
                TrackEvent::Pause.into(),
                TrackEventHandler {
                    track_meta: Arc::clone(&track_meta),
                },
            )?;

            t_handle.add_event(
                TrackEvent::End.into(),
                TrackEventHandler {
                    track_meta: Arc::clone(&track_meta),
                },
            )?;

            t_handle.add_event(
                TrackEvent::Error.into(),
                TrackErrorHandler {
                    track_meta: Arc::clone(&track_meta),
                },
            )?;
        }

    }

    Ok(())
}
