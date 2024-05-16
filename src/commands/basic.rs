use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;
use poise::serenity_prelude::{
    CreateInteractionResponse, CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
    EditMessage,
};
use tracing::{info, instrument};

#[poise::command(slash_command, prefix_command)]
#[instrument(name = "ping", skip_all)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!("Ping! {} ms", ctx.ping().await.as_millis()))
        .await?;

    info!("Johnson pinged {}", ctx.author().name);

    Ok(())
}

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn test_interaction(ctx: Context<'_>) -> Result<(), Error> {
    let interaction = match ctx {
        Context::Application(a) => a.interaction,
        _ => {
            panic!("Impossible")
        }
    };

    interaction
        .create_response(
            ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("Hello World"),
            ),
        )
        .await?;

    interaction
        .create_followup(
            ctx,
            CreateInteractionResponseFollowup::new().content("Hello World"),
        )
        .await?;

    Ok(())
}
