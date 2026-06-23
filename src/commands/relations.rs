use poise::CreateReply;
use poise::serenity_prelude::{Color, CreateEmbed, Http, Mentionable, User, UserId};
use tracing::instrument;

use crate::custom_types::command::{Context, Error};
use crate::db::{Database, RelationType};
use crate::utils::message::embed::base_embed;

#[poise::command(slash_command, prefix_command)]
#[instrument(skip(ctx))]
pub async fn add_friend(ctx: Context<'_>, new_friend: User) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let relation_to = db_handler.get_relation(ctx.author(), &new_friend).await?;
    let relation_from = db_handler.get_relation(&new_friend, ctx.author()).await?;

    if relation_to == Some(RelationType::Friend) {
        ctx.say("You are already friends!").await?;
        return Ok(());
    }

    db_handler.add_friend(ctx.author(), &new_friend).await?;

    match relation_from {
        Some(r_from) => match r_from {
            RelationType::Friend => {
                ctx.say(format!(
                    "{} and {} are now both friends!",
                    ctx.author().mention(),
                    new_friend.mention()
                ))
                .await?;
            }
            RelationType::Blocked => {
                ctx.say(format!(
                    "{} has you blocked....this is kind of awkward :/",
                    new_friend.mention()
                ))
                .await?;
            }
            _ => {
                ctx.say(format!(
                    "{} doesn't really know you...let's see if they wanna be friends",
                    new_friend.mention()
                ))
                .await?;
            }
        },
        None => {
            ctx.say(format!(
                "{} doesn't really know you...let's see if they wanna be friends",
                new_friend.mention()
            ))
            .await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
#[instrument(skip(ctx))]
pub async fn block_user(ctx: Context<'_>, blocked: User) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let already_added =
        db_handler.get_relation(ctx.author(), &blocked).await? == Some(RelationType::Blocked);

    if already_added {
        ctx.say(format!("{} is already blocked!", blocked.name))
            .await?;
    } else {
        db_handler.block_user(ctx.author(), &blocked).await?;
        ctx.say(format!("You have blocked {}", blocked.mention()))
            .await?;
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
#[instrument(skip(ctx))]
pub async fn get_relationships(ctx: Context<'_>) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let relations = db_handler.get_relations(ctx.author()).await?;

    let mut friend_embed = base_embed().title("Friends").color(Color::DARK_GREEN);
    let mut enemies_embed = base_embed().title("Enemies (Blocked)").color(Color::RED);
    let mut broken_embed = base_embed()
        .title("This should never appear. Please send help.")
        .color(Color::DARK_PURPLE);
    let mut friends = Vec::new();
    let mut blocked = Vec::new();
    let mut invalid = Vec::new();

    for relation in relations {
        match relation.1 {
            RelationType::Friend => friends.push(relation.0),
            RelationType::Blocked => blocked.push(relation.0),
            RelationType::Invalid => invalid.push(relation.0),
        }
    }

    friend_embed = display_users_embed(ctx.http(), &friends, friend_embed).await;
    enemies_embed = display_users_embed(ctx.http(), &blocked, enemies_embed).await;
    broken_embed = display_users_embed(ctx.http(), &invalid, broken_embed).await;

    if !friends.is_empty() {
        ctx.send(
            CreateReply::default()
                .embed(friend_embed)
                .reply(true)
                .ephemeral(true),
        )
        .await?;
    }

    if !blocked.is_empty() {
        ctx.send(
            CreateReply::default()
                .embed(enemies_embed)
                .reply(true)
                .ephemeral(true),
        )
        .await?;
    }

    if !invalid.is_empty() {
        ctx.send(
            CreateReply::default()
                .embed(broken_embed)
                .reply(true)
                .ephemeral(true),
        )
        .await?;
    }

    if friends.is_empty() && blocked.is_empty() && invalid.is_empty() {
        ctx.send(
            CreateReply::default()
                .content("You have nothing")
                .ephemeral(true),
        )
        .await?;
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
#[instrument(skip(ctx))]
pub async fn unfriend(ctx: Context<'_>, friend: User) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    db_handler.remove_relation(ctx.author(), &friend).await?;

    ctx.say(format!("Unfriended {}", friend.mention())).await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn unblock(ctx: Context<'_>, loser: User) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    db_handler.remove_relation(ctx.author(), &loser).await?;

    ctx.say(format!("Unblocked {}", loser.mention())).await?;

    Ok(())
}

async fn display_users_embed(http: &Http, users: &[UserId], mut embed: CreateEmbed) -> CreateEmbed {
    for user in users {
        embed = embed.field(
            format!("{user}"),
            user.to_user(http)
                .await
                .map_or(String::from("Invalid User"), |user| user.name),
            false,
        )
    }

    embed
}
