use poise::serenity_prelude::Role;

use crate::custom_types::command::{Context, Error};
use crate::db::Database;

#[poise::command(slash_command)]
pub async fn set_welcome_role(ctx: Context<'_>, welcome_role: Role) -> Result<(), Error> {
    let client = Database::new(ctx);
    let guild_id = ctx.guild_id();

    if let Some(guild_id) = guild_id {
        client.save_welcome_role(guild_id, welcome_role.id).await?;
        ctx.reply("Set role!").await?;
    } else {
        ctx.reply("Can't use this outside of a guild").await?;
    }

    Ok(())
}
