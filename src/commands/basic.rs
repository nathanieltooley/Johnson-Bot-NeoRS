use crate::custom_types::command::{Context, Error};
use tracing::{debug, info, instrument};

#[poise::command(slash_command, prefix_command)]
#[instrument(name = "ping", skip_all)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Ping!").await?;

    info!("Johnson pinged {}", ctx.author().name);

    Ok(())
}
