use poise::CreateReply;
use poise::serenity_prelude::User;

use crate::custom_types::command::{Context, Error};
use crate::db::Database;
use crate::utils::message::embed::base_embed;

#[poise::command(slash_command, prefix_command)]
pub async fn add_friend(ctx: Context<'_>, new_friend: User) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    db_handler.add_friend(ctx.author(), &new_friend).await?;

    ctx.say("Added friend").await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn get_friends(ctx: Context<'_>) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let friends = db_handler.get_friends(ctx.author()).await?;

    let mut friend_embed = base_embed().title("Friends");
    for friend in friends {
        friend_embed = friend_embed.field("Friend", format!("{friend}"), false);
    }

    ctx.send(CreateReply::default().embed(friend_embed).reply(true))
        .await?;
    Ok(())
}
