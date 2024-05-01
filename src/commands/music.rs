use poise::serenity_prelude;
use tracing::debug;

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn play(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.guild().is_none() {
        ctx.reply("Cannot run this command outside of a guild")
            .await?;
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

    Ok(())
}
