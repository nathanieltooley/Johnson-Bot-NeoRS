use poise::serenity_prelude::{CreateMessage, Message, Result};

use crate::custom_types::command::Context;

pub fn simple_message(content: &str) -> CreateMessage {
    CreateMessage::new().content(content)
}

pub async fn simple_channel_message(ctx: &Context<'_>, content: &str) -> Result<Message> {
    ctx.channel_id()
        .send_message(ctx, simple_message(content))
        .await
}

pub mod interaction {
    use futures::StreamExt;
    use poise::serenity_prelude::{
        ComponentInteraction, CreateInteractionResponse, CreateInteractionResponseMessage, Message,
        UserId,
    };
    use std::time::Duration;

    use crate::custom_types::command::{Context, Error};

    pub async fn wait_for_user_interaction(
        ctx: &Context<'_>,
        message: &Message,
        comp_id: UserId,
        timeout: Duration,
    ) -> Option<ComponentInteraction> {
        let mut message_stream = message
            .await_component_interaction(ctx)
            .timeout(timeout)
            .stream();

        while let Some(interaction) = message_stream.next().await {
            let user_id = interaction.user.id;

            if user_id != comp_id {
                let _ = interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("This is not meant for you!")
                                .ephemeral(true),
                        ),
                    )
                    .await;
            } else {
                return Some(interaction);
            }
        }

        None
    }
}
