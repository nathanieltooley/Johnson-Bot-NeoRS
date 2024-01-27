use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    options::ClientOptions,
    Client, Collection, Database,
};
use poise::serenity_prelude::GuildId;
use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, MutexGuard};
use tracing_subscriber::registry::Data;

use crate::custom_types::mongo_schema::User;
pub async fn receive_client(mongo_uri: &str) -> Result<Client, mongodb::error::Error> {
    let mut client_options = ClientOptions::parse(mongo_uri).await?;
    client_options.app_name = Some("Johnson Bot RS".to_string());

    Client::with_options(client_options)
}

async fn get_user_collection(
    db_handle: &MutexGuard<'_, Database>,
    guild_id: GuildId,
) -> Collection<User> {
    db_handle.collection(&guild_id.to_string())
}

pub async fn get_all_docs<T>(coll: &Collection<T>) -> Result<Vec<T>, mongodb::error::Error>
where
    // The Stream trait (and by extension TryStreamExt) is only implemented on cursors
    // for structs that implement these traits
    // https://www.mongodb.com/community/forums/t/rust-driver-help-writing-a-generic-find-method/168846/2
    T: DeserializeOwned + Sync + Send + Unpin,
{
    let cursor = coll.find(doc! {}, None).await?;
    cursor.try_collect::<Vec<T>>().await
}

/// Returns the users of a given server in the DB, or an error
///
/// This function requires a MutexGuard and will not drop it until the end of the function
///
pub async fn get_users<'a>(
    guild_id: GuildId,
    handle: MutexGuard<'_, Database>,
) -> Result<Vec<User>, mongodb::error::Error> {
    // Give a reference to a MutexGuard rather than the guard so that we keep the guard
    // thus making sure this collection will not change until we grab all the users
    let user_col = get_user_collection(&handle, guild_id).await;
    get_all_docs(&user_col).await
}
