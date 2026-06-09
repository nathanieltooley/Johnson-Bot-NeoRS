use poise::CreateReply;
use poise::serenity_prelude::{Mentionable, User};

use crate::custom_types::command::{Context, Error};
use crate::db::Database;
use crate::utils::message::embed::base_embed;

#[poise::command(slash_command, prefix_command)]
pub async fn add_friend(ctx: Context<'_>, new_friend: User) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let already_added = db_handler.add_friend(ctx.author(), &new_friend).await?;

    if already_added {
        ctx.say(format!("{} is already your friend!", new_friend.name))
            .await?;
    } else {
        ctx.say(format!("You are now friends with {}", new_friend.mention()))
            .await?;
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn get_friends(ctx: Context<'_>) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let friends = db_handler.get_friends(ctx.author()).await?;

    let mut friend_embed = base_embed().title("Friends");
    for friend in friends {
        let friend_name = friend.to_user(ctx.http()).await?;
        friend_embed = friend_embed.field("Friend", format!("{friend_name}"), false);
    }

    ctx.send(CreateReply::default().embed(friend_embed).reply(true))
        .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn remove_friend(ctx: Context<'_>, friend: User) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    db_handler.remove_friend(ctx.author(), &friend).await?;

    ctx.say("Removed friend").await?;

    Ok(())
}
