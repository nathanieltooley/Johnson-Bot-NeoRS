use poise::serenity_prelude::{async_trait, ChannelId, GuildId};
use songbird::input::{AuxMetadata, Compose, YoutubeDl};
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler, TrackEvent};
use tokio::sync::{Mutex, MutexGuard};
use tracing::{debug, error, info};
use url::{Host, Url};

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Arc;

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;

static DRIVER_EVENTS_ADDED: AtomicBool = AtomicBool::new(false);

struct DriverReconnectHandler;
struct DriverDisconnectHandler;

struct TrackEventHandler {
    track_meta: AuxMetadata,
}
struct TrackErrorHandler {
    track_meta: AuxMetadata,
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
            for (t_state, _t_handle) in *track_list {
                // &Option<String> -> Option<&String>
                let title = self.track_meta.title.as_deref().unwrap_or("No Title");
                let artist = self.track_meta.artist.as_deref().unwrap_or("No Artist");

                info!(
                    "Track {} : {} updated to state: {:?}",
                    title, artist, t_state
                );
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
                let title = self.track_meta.title.as_deref().unwrap_or("No Title");
                let artist = self.track_meta.artist.as_deref().unwrap_or("No Artist");

                info!(
                    "Track {} : {} Encountered an error: {:?}",
                    title, artist, t_state.playing
                )
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
            ctx.reply(format!("Could not parse URL: {e}")).await?;
            return Ok(());
        }
    };

    if parsed_url.host() != Some(Host::Domain("youtube.com"))
        && parsed_url.host() != Some(Host::Domain("youtu.be"))
    {
        ctx.reply("Invalid URL").await?;
        return Ok(());
    }

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

    debug!("Attempting to join VC");

    let vc = join(&ctx, guild_id, channel_id).await?;

    {
        let mut h_lock = vc.lock().await;

        if !DRIVER_EVENTS_ADDED.load(Relaxed) {
            debug!("Attaching event handlers to global driver");
            attach_event_handlers(&mut h_lock).await;
            DRIVER_EVENTS_ADDED.swap(true, Relaxed);
        }

        let http_client = ctx.data().http.clone();

        let mut ytdl = YoutubeDl::new(http_client, url);
        let meta = ytdl.aux_metadata().await?;

        let meta_events = meta.clone();

        let title = &meta.title.unwrap_or(String::from("No Title"));
        let author = &meta.artist.unwrap_or(String::from("No Author"));

        debug!("Playing {}, by {}", title, author);
        let t_handle = h_lock.play_input(ytdl.into());

        t_handle.add_event(
            TrackEvent::Pause.into(),
            TrackEventHandler {
                track_meta: meta_events.clone(),
            },
        )?;

        t_handle.add_event(
            TrackEvent::End.into(),
            TrackEventHandler {
                track_meta: meta_events.clone(),
            },
        )?;

        t_handle.add_event(
            TrackEvent::Error.into(),
            TrackErrorHandler {
                track_meta: meta_events.clone(),
            },
        )?;

        ctx.reply("Playing song").await?;
    }

    debug!("Done loading song");

    Ok(())
}
