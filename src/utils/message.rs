use poise::serenity_prelude::{CreateMessage, Message, Result};

use crate::custom_types::command::Context;

pub fn simple_message(content: &str) -> CreateMessage {
    CreateMessage::new().content(content)
}

pub async fn send_channel_message(ctx: &Context<'_>, content: &str) -> Result<Message> {
    ctx.channel_id()
        .send_message(ctx, simple_message(content))
        .await
}
