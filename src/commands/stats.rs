use poise::serenity_prelude::{CreateEmbed, CreateEmbedAuthor, Timestamp};
use poise::CreateReply;

use crate::custom_types::command::{Context, Error};
use crate::db::Database;
use crate::events::error_handle;

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn show_stats(ctx: Context<'_>) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let user_info = db_handler.get_user(ctx.author()).await?;

    let stat_fields = vec![
        ("XP", format!("{}", user_info.exp), false),
        ("V-Bucks", format!("{}", user_info.vbucks), false),
    ];

    let stats_embed = CreateEmbed::new()
        .title(format!("{}'s Stats", ctx.author().name))
        .author(CreateEmbedAuthor::new("Johnson Bot"))
        .fields(stat_fields)
        .timestamp(Timestamp::now());

    ctx.send(
        CreateReply::default()
            .embed(stats_embed)
            .reply(true)
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
