use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    options::ClientOptions,
    Client, Collection, Database,
};
use poise::serenity_prelude::GuildId;
use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, MutexGuard};
use tracing::instrument;

use crate::custom_types::{
    command::{DBInfo, JohnsonDBHandle},
    mongo_schema::User,
};

#[instrument]
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
/// ~~~
/// let users = mongo::get_users(DBInfo::Event(&ctx, guild_id)).await;
///
/// // Print out the values if it succeeded
/// match users {
///     Ok(users) => {
///         for user in users {
///              debug!("User: {:?}", user);
///         }
///     }
///     Err(e) => {
///         error!("{}", e);
///     }
/// }
/// ~~~
pub async fn get_users<'a>(db_info: DBInfo<'_>) -> Result<Vec<User>, mongodb::error::Error> {
    match db_info {
        DBInfo::PoiseContext(ctx) => {
            let guild_id = ctx.guild_id().unwrap();
            let handle = ctx.data().johnson_handle.lock().await;

            // Give a reference to a MutexGuard rather than the guard so that we keep the guard
            // thus making sure this collection will not change until we grab all the users
            let user_col = get_user_collection(&handle, guild_id).await;
            get_all_docs(&user_col).await
        }
        DBInfo::Event(ctx, guild_id) => {
            let ctx_data = ctx.data.read().await;
            let handle = ctx_data
                .get::<JohnsonDBHandle>()
                .expect("Johnson expected context data to hold DB Handle")
                .lock()
                .await;

            let user_col = get_user_collection(&handle, guild_id).await;
            get_all_docs(&user_col).await
        }
    }
}
