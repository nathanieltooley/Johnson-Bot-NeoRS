use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    async_trait, ChannelId, Color, CreateEmbed, CreateEmbedFooter, CreateMessage, GuildId, Http, Message
};
use poise::CreateReply;
use rspotify::model::FullTrack;
use songbird::input::{AudioStreamError, Compose, YoutubeDl};
use songbird::tracks::{PlayMode, Track};
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler, TrackEvent};
use tokio::sync::{Mutex, MutexGuard};
use tracing::field::debug;
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

use std::cmp::min;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;
use std::time::Instant;
use rand::{thread_rng, Rng};

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;
use crate::spotify::get_tracks_from_url;

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

static LAST_NOW_PLAYING_MESSAGE: Lazy<Mutex<Option<Message>>> = Lazy::new(|| Mutex::new(None)); 

#[derive(Clone)]
struct TrackMetadata {
    title: String,
    artists: String,
    r#type: TrackType,
    domain: Option<String>,
    img_url: Option<String>
}

#[derive(Clone, PartialEq)]
enum TrackType {
    Ytdl,
    Spotify
}

struct TrackWrapper {
    ytdl: YoutubeDl,
    metadata: TrackMetadata
}

impl TrackWrapper {
    pub async fn from_ytdl(mut ytdl: YoutubeDl, url: &Url) -> Result<TrackWrapper, AudioStreamError> {
        let meta = ytdl.aux_metadata().await?;

        let title = meta.title.unwrap_or(String::from("No Title Found")); 
        let artists = meta.artist.unwrap_or(String::from("No Artists Found"));
        let r#type = TrackType::Ytdl;
        let domain = url.domain().map(str::to_string);
        let img_url = meta.thumbnail;

        let metadata = TrackMetadata {
            title,
            artists,
            r#type,
            domain,
            img_url
        };

        Ok(TrackWrapper { ytdl, metadata })
    }

    pub fn from_spotify(track: FullTrack, http_client: reqwest::Client, url: &Url) -> TrackWrapper {
        let title = track.name;
        let artists = track.artists.iter().map(|artist| &artist.name).map(String::as_str).collect::<Vec<_>>().join(" ");
        let r#type = TrackType::Spotify;
        let domain = url.domain().map(str::to_string);
        let img_url = track.album.images.into_iter().max_by(|i, i2| {
            i.height.unwrap_or(0).cmp(&i2.height.unwrap_or(0))
        }).map(|img| img.url);

        let mut search_string = String::new();
        search_string.push_str(&format!("{} {}", title, artists));

        let metadata = TrackMetadata {
            title,
            artists,
            r#type,
            domain,
            img_url
        };


        let ytdl = YoutubeDl::new_search(http_client, search_string);

        TrackWrapper {ytdl, metadata}
    }
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
                let title = &self
                    .track_meta
                    .title;

                let artist = &self
                    .track_meta
                    .artists;

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
                let title = &self
                    .track_meta
                    .title;

                let artist = &self
                    .track_meta
                    .artists;

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

                match track_message_result {
                    Err(e) => error!("Could not send track play notification message {e:?}"),
                    Ok(message) => {
                        // Delete and replace previous now playing message
                        if Lazy::get(&LAST_NOW_PLAYING_MESSAGE).is_some() {
                            let mut lock = LAST_NOW_PLAYING_MESSAGE.lock().await;
                            let last_message = lock.as_mut().unwrap();
                            let _ = last_message.delete(&self.http_handle).await;
                        } 

                        *LAST_NOW_PLAYING_MESSAGE.lock().await = Some(message);                            
                    }
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
            metadata.domain.as_deref().unwrap_or("")
        )
        .unwrap_or(&Color::GOLD);
    
    let embed = CreateEmbed::new()
        .title(format!("NOW PLAYING: {}", metadata.title))
        .field(
            "Song Artist: ",
            &metadata.artists,
            false,
        )
        .color(color);

    // Add a disclamier
    let embed =  {
        if metadata.r#type == TrackType::Spotify {
            embed.footer(CreateEmbedFooter::new("NOTE: THIS SONG MAY BE WRONG AS IT WAS TAKEN FROM YOUTUBE RATHER THAN SPOTIFY DIRECTLY"))
        } else {
            embed
        }
    };

    // If the metadata contains a thumbnail, create an embed thumbnail
    match &metadata.img_url {
        Some(thumbnail) => embed.image(thumbnail),
        None => embed,
    }
}

#[poise::command(slash_command, on_error = "error_handle", guild_only)]
pub async fn play(ctx: Context<'_>, url: String) -> Result<(), Error> {
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

    let mut ytdl_tracks: Vec<TrackWrapper> = Vec::new();

    match parsed_url.host() {
        Some(h) => {
            match h.to_string().as_str() {
                "youtube.com" | "youtu.be" => {
                    debug!("User passed in Youtube link");

                    let http_client = ctx.data().http.clone();

                    ytdl_tracks.push(TrackWrapper::from_ytdl(YoutubeDl::new(http_client, url), &parsed_url).await?);
                }
                "open.spotify.com" => {
                    let spotify = &ctx.data().spotify_client;
                    let http_client = &ctx.data().http;
                    let tracks = get_tracks_from_url(spotify, &parsed_url).await?;

                    debug!("Enqueuing {} tracks from spotify", tracks.len());

                    ctx.say(format!("Enqueuing {} tracks from Spotify", tracks.len())).await?;

                    ytdl_tracks.append(&mut tracks.into_iter().map(|track| {
                        TrackWrapper::from_spotify(track, http_client.clone(), &parsed_url)
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

        for ytdl in ytdl_tracks {

            // let start = Instant::now();

            let title = &ytdl.metadata.title;
            let author = &ytdl.metadata.artists;

            // Create track here so we can get the UUID
            // and insert it into the map before it gets played
            
            let track = Track::new(ytdl.ytdl.into());
            let track_meta = Arc::new(ytdl.metadata.clone());


            TRACK_METADATA_MAP
                .lock()
                .await
                .insert(track.uuid, track_meta.clone());

            let q = Instant::now();
            let t_handle = h_lock.enqueue_with_preload(track, None);

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

            // debug!("Took {} seconds to queue song", start.elapsed().as_secs());
        }

    }

    Ok(())
}

#[poise::command(slash_command, on_error = "error_handle", guild_only)]
pub async fn skip(ctx: Context<'_>, count: Option<u32>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    ctx.defer().await?;

    let manager = songbird::get(ctx.serenity_context()).await.expect("songbird should be initialized");

    if let Some(call) = manager.get(guild_id) {
        let handler = call.lock().await;
        let queue = handler.queue();

        let _ = queue.skip();

        let count = count.unwrap_or(1);

        handler.queue().modify_queue(|m_q| {
            for _ in 0..count {
                // if queue.is_empty() {
                //     break;
                // }

                let track = m_q.pop_front().unwrap();
                debug!("Skipped track: {:?}", track);
            }
        });

        queue.resume()?;

        ctx.say("Skipped current song!").await?;
    } else {
        ctx.say("Not currently in a voice channel!").await?;
    }

    Ok(())
}

#[poise::command(slash_command, on_error = "error_handle", guild_only)]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context()).await.expect("songbird should be initialized");

    if let Some(call) = manager.get(guild_id) {
        let lock = call.lock().await;
        let queue = lock.queue();
        let _ = queue.pause();

        ctx.say("Paused current song!").await?;
    } else {
        ctx.say("Not currently in a voice channel").await?;
    }

    Ok(())
}

#[poise::command(slash_command, on_error = "error_handle", guild_only)]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context()).await.expect("songbird should be initialized");

    if let Some(call) = manager.get(guild_id) {
        let lock = call.lock().await;
        let _ = lock.queue().resume();

        ctx.say("Resuming current song!").await?;
    }

    Ok(())
}

#[poise::command(slash_command, on_error = "error_handle", guild_only)]
pub async fn queue(
    ctx: Context<'_>, 
    #[max = 10]
    count: Option<usize>) -> Result<(), Error> 
{
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context()).await.expect("songbird should be initialized");

    ctx.defer().await?;

    if let Some(call) = manager.get(guild_id) {
        let lock = call.lock().await;
        let queue = lock.queue().current_queue();

        if queue.is_empty() {
            ctx.say("Queue is empty!").await?;
        }

        ctx.say(format!("Queue contains {} songs", queue.len())).await?;

        let count = min(count.unwrap_or(10), queue.len());

        let map_lock = TRACK_METADATA_MAP.lock().await;

        let embed = CreateEmbed::new().title("Queue");
        let mut description = String::new();

        description.push_str("`Now Playing => ");
        (0..count).for_each(|i| {
            let meta = map_lock.get(&queue[i].uuid());
            let default_string = String::from("No Metadata");
            let title = meta.map(|m| &m.title).unwrap_or(&default_string); 
            let artists = meta.map(|m| &m.artists).unwrap_or(&default_string);

            description.push_str(&format!("{} - {}\n", title, artists)); 
        });

        description.push('`');
        let embed = embed.description(description);

        ctx.send(CreateReply { content: None, embeds: vec![embed], ..Default::default()}).await?;
    }

    Ok(())
}

#[poise::command(slash_command, on_error = "error_handle", guild_only)]
pub async fn shuffle(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let _ = ctx.defer().await;

    let manager = songbird::get(ctx.serenity_context()).await.expect("songbird should be initialized");

    if let Some(call) = manager.get(guild_id) {
        let lock = call.lock().await;
        let queue_len = lock.queue().len();
        lock.queue().modify_queue(|queue| {
            // Fisher-Yates shuffle
            // start at one to ignore the first song in the queue (the currently playing song)
            for i in 1..queue_len {
                let mut rand = thread_rng();
                let r_index = rand.gen_range(1..queue.len()); 
                
                queue.swap(i, r_index);
            }
        });

        ctx.say(format!("Shuffled {} songs!", queue_len)).await?;
    } else {
        ctx.say("There is no queue to shuffle!").await?;
    }

    Ok(())
}
