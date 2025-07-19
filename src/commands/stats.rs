use poise::CreateReply;

use crate::custom_types::command::{Context, Error};
use crate::db::Database;
use crate::events::error_handle;
use crate::utils::message::embed::base_embed;

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn show_stats(
    ctx: Context<'_>,
    #[description = "let the whole world know?"] annoy_others: bool,
) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let user_info = db_handler.get_user(ctx.author()).await?;

    let stat_fields = vec![
        ("XP", format!("*{}*", user_info.exp), false),
        ("V-Bucks", format!("*{}*", user_info.vbucks), false),
    ];

    let stats_embed = base_embed()
        .title(format!("{}'s Stats", ctx.author().name))
        .fields(stat_fields);

    ctx.send(
        CreateReply::default()
            .embed(stats_embed)
            .reply(true)
            .ephemeral(!annoy_others),
    )
    .await?;
    Ok(())
}
