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

use crate::custom_types::{command::DataMongoClient, mongo_schema::User};

static DB_NAME: &str = "Johnson";

#[instrument]
pub async fn receive_client(mongo_uri: &str) -> Result<Client, mongodb::error::Error> {
    let mut client_options = ClientOptions::parse(mongo_uri).await?;
    client_options.app_name = Some("Johnson Bot RS".to_string());

    Client::with_options(client_options)
}

async fn get_user_collection(mongo_client: &Client, guild_id: GuildId) -> Collection<User> {
    mongo_client
        .database(DB_NAME)
        .collection(&guild_id.to_string())
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

pub async fn get_users<'a>(
    mongo_client: &'a Client,
    guild_id: GuildId,
) -> Result<Vec<User>, mongodb::error::Error> {
    let user_col = get_user_collection(mongo_client, guild_id).await;

    get_all_docs(&user_col).await
}
