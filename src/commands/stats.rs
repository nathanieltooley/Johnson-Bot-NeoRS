use poise::serenity_prelude::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, Timestamp};
use poise::CreateReply;

use crate::custom_types::command::{Context, Error};
use crate::db::Database;
use crate::events::error_handle;

static JBOT_PFP_URL: &str = "https://cdn.discordapp.com/attachments/1276784436494733384/1290877955656122419/Worship.png?ex=687b10c7&is=6879bf47&hm=be23b2e97af43997c6b6001992f096b5220f60ff5b9ae8ddf3be1c6b54a1685f&";

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

    let stats_embed = CreateEmbed::new()
        .title(format!("{}'s Stats", ctx.author().name))
        .author(CreateEmbedAuthor::new("Johnson Bot").icon_url(JBOT_PFP_URL))
        .fields(stat_fields)
        .footer(CreateEmbedFooter::new("written by beanbubger"))
        .timestamp(Timestamp::now());

    ctx.send(
        CreateReply::default()
            .embed(stats_embed)
            .reply(true)
            .ephemeral(!annoy_others),
    )
    .await?;
    Ok(())
}
