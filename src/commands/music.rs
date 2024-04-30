use poise::serenity_prelude;
use tracing::debug;

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.guild().is_none() {
        return Ok(());
    }

    let guild_id = ctx.guild_id().unwrap();
    // Get the VC the user is connected to
    let channel_id = ctx
        .guild()
        .unwrap() // TODO: Remove unwrap
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id)
        .unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird should be registered with Johnson Bot");

    match manager.join(guild_id, channel_id).await {
        Ok(_) => {
            debug!("Johnson joined a voice channel");
        }
        Err(e) => {
            debug!("Johnson failed to join a voice channel {e}");
            // Return join error
            return Err(Box::new(e));
        }
    }

    Ok(())
}
