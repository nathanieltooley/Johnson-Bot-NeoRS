use poise::serenity_prelude::Role;

use crate::custom_types::command::{Context, Error};
use crate::events::error_handle;
use crate::mongo::ContextWrapper;

#[poise::command(slash_command, on_error = "error_handle")]
pub async fn set_welcome_role(ctx: Context<'_>, welcome_role: Role) -> Result<(), Error> {
    let client = ContextWrapper::new_slash(ctx);
    client.save_welcome_role(welcome_role.id).await?;

    Ok(())
}
