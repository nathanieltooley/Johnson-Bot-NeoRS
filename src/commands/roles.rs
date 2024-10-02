use poise::serenity_prelude::Role;

use crate::custom_types::command::{Context, Error, PartialData, SerenityCtxData};
use crate::events::error_handle;

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn set_welcome_role(ctx: Context<'_>, welcome_role: Role) -> Result<(), Error> {
    let mut data_lock = ctx.serenity_context().data.write().await;
    let data = data_lock
        .get_mut::<SerenityCtxData>()
        .expect("data should be initialized");
    data.welcome_role = Some(welcome_role);

    Ok(())
}
