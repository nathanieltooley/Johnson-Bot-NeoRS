use poise::{CreateReply, serenity_prelude::ChannelId};

use crate::{
    custom_types::command::{Context, Error},
    db::Database,
};

#[poise::command(slash_command)]
pub async fn view_server_conf(ctx: Context<'_>) -> Result<(), Error> {
    let db = Database::new(ctx);
    let server_conf = db
        .get_server_conf(ctx.guild_id().expect("not used in DM"))
        .await?;

    // TODO: Make this nicer to look at
    ctx.send(
        CreateReply::default()
            .ephemeral(true)
            .content(format!("{server_conf:?}")),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn set_error_channel(ctx: Context<'_>, channel_id: ChannelId) -> Result<(), Error> {
    let db = Database::new(ctx);
    db.save_error_channel(ctx.guild_id().expect("not used in DM"), channel_id)
        .await?;

    ctx.send(
        CreateReply::default()
            .reply(true)
            .content(format!("Set {channel_id} as error channel id!")),
    )
    .await?;

    Ok(())
}
