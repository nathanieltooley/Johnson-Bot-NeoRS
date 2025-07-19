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

    use crate::custom_types::command::Context;

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

pub mod embed {
    use poise::serenity_prelude::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, Timestamp};

    static JBOT_PFP_URL: &str = "https://cdn.discordapp.com/attachments/1276784436494733384/1290877955656122419/Worship.png?ex=687b10c7&is=6879bf47&hm=be23b2e97af43997c6b6001992f096b5220f60ff5b9ae8ddf3be1c6b54a1685f&";

    pub fn base_embed() -> CreateEmbed {
        CreateEmbed::new()
            .author(CreateEmbedAuthor::new("Johnson Bot").icon_url(JBOT_PFP_URL))
            .footer(CreateEmbedFooter::new("written by beanbubger"))
            .timestamp(Timestamp::now())
    }
}
