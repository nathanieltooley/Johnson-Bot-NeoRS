use crate::custom_types::{Context, Data, Error};
use tracing::{info, instrument};

#[poise::command(slash_command, prefix_command)]
#[instrument(name = "ping", skip_all, fields(data = ?ctx.data()))]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Ping!").await?;
    info!("Johnson pinged {}", ctx.author().name);
    Ok(())
}
