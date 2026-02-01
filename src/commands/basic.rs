#![allow(clippy::derived_hash_with_manual_eq)]
use crate::{
    built_info,
    custom_types::command::{Context, Error},
    events::error_handle,
    utils::message::embed::base_embed,
};
// use crate::events::error_handle;
use poise::{
    CreateReply,
    serenity_prelude::{
        CreateInteractionResponse, CreateInteractionResponseFollowup,
        CreateInteractionResponseMessage,
    },
};
use problemo::static_gloss_error;
use tracing::{info, instrument};

#[poise::command(slash_command, prefix_command)]
#[instrument(name = "ping", skip_all)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!("Ping! {} ms", ctx.ping().await.as_millis()))
        .await?;

    info!("Johnson pinged {}", ctx.author().name);

    Ok(())
}

#[poise::command(slash_command)]
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
            CreateInteractionResponseFollowup::new().content("Goodbye World!"),
        )
        .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn version(
    ctx: Context<'_>,
    #[description = "Tell the whole world?"] annoy_others: bool,
) -> Result<(), Error> {
    let version_embed = base_embed().title(built_info::GIT_VERSION.unwrap().to_owned());
    ctx.send(
        CreateReply::default()
            .embed(version_embed)
            .ephemeral(!annoy_others),
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn smile(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(CreateReply::default().reply(true).content("https://cdn.discordapp.com/attachments/322818382506229768/1409415101450293278/Johnson_Smile.png?ex=68ad4b99&is=68abfa19&hm=9add5e51f18ee34b825705d8c142e22380dab4f42c98619d420be05b3b4c03cc")).await?;
    Ok(())
}

static_gloss_error!(TestError, "This is a test");
static_gloss_error!(TestError2, "This is the next test");

#[poise::command(slash_command)]
pub async fn test_problem(ctx: Context<'_>) -> Result<(), Error> {
    Err(TestError::as_problem("there is more testing")
        .via(TestError2::new("There is now a second test")))
}
