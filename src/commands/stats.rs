use poise::CreateReply;
use poise::serenity_prelude::User;

use crate::custom_types::command::{Context, Error};
use crate::db::Database;
use crate::utils::message::embed::base_embed;

#[poise::command(slash_command)]
pub async fn show_stats(
    ctx: Context<'_>,
    #[description = "Let the whole world know?"] annoy_others: bool,
    #[description = "Who's stats would you like to see. By default this is you."]
    user_to_show: Option<User>,
) -> Result<(), Error> {
    let db_handler = Database::new(ctx);
    let user = user_to_show.unwrap_or_else(|| ctx.author().to_owned());
    let user_info = db_handler.get_user(&user).await?;

    let stat_fields = vec![
        ("XP", format!("*{}*", user_info.exp), false),
        ("V-Bucks", format!("*{}*", user_info.vbucks), false),
    ];

    let stats_embed = base_embed()
        .title(format!("{}'s Stats", user.name))
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
