use crate::custom_types::command::Context;
use mongodb::Database;
use poise::serenity_prelude::GuildId;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Returns relevant DB info from a poise::Context object
///
/// This function does clone the Arc inside the Context but this should not result in too much of a cost
pub fn ctx_db_info(ctx: &Context) -> (GuildId, Arc<Mutex<Database>>) {
    (
        ctx.guild_id().unwrap(),
        Arc::clone(&ctx.data().johnson_handle),
    )
}
