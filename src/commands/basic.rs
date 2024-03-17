use crate::custom_types::command::{Context, Data, Error};
use tracing::{debug, info, instrument};

#[poise::command(slash_command, prefix_command)]
#[instrument(name = "ping", skip_all)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!("Ping! {} ms", ctx.ping().await.as_millis()))
        .await?;

    info!("Johnson pinged {}", ctx.author().name);

    Ok(())
}
