use songbird::input::cached::Compressed;
use songbird::input::{File, YoutubeDl};
use tracing::{debug, error};
use url::{Host, Url};

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn play(ctx: Context<'_>, url: String) -> Result<(), Error> {
    if ctx.guild().is_none() {
        ctx.reply("Cannot run this command outside of a guild")
            .await?;
        return Ok(());
    }

    let parsed_url = match Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            error!("Could not parse URL: {e}");
            ctx.reply("Could not parse URL: {e}").await?;
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

    ctx.defer_ephemeral().await?;

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird should be registered with Johnson Bot")
        .clone();

    let vc = match manager.get(guild_id) {
        Some(c) => c,
        None => {
            // If the bot is not in a call, join the user's channel
            match manager.join(guild_id, channel_id).await {
                Ok(c) => {
                    debug!("Johnson joined a voice channel");
                    c
                }
                Err(e) => {
                    debug!("Johnson failed to join a voice channel {e}");
                    // Return join error
                    return Err(Box::new(e));
                }
            }
        }
    };

    {
        let mut h_lock = vc.lock().await;
        let http_client = ctx.data().http.clone();

        // let song_src = Compressed::new(
        //     File::new("resources/Apoapsis v6.wav").into(),
        //     songbird::driver::Bitrate::Auto,
        // )
        // .await
        // .unwrap();
        //
        // h_lock.play_input(song_src.into());
        //

        let ytdl = YoutubeDl::new(http_client, url);
        h_lock.play_input(ytdl.into());

        ctx.reply("Playing song").await?;
    }

    Ok(())
}
