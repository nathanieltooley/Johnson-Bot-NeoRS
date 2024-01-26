use crate::custom_types::command::{Context, Error};
use crate::mongo::get_users;
use tracing::{debug, info, instrument};

#[poise::command(slash_command, prefix_command)]
#[instrument(name = "ping", skip_all)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Ping!").await?;

    for user in get_users(&ctx).await? {
        debug!("User: {:?}", user);
    }

    info!("Johnson pinged {}", ctx.author().name);

    Ok(())
}
