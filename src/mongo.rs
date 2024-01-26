use futures::stream::{StreamExt, TryStreamExt};
use mongodb::{
    bson::{doc, Document},
    options::ClientOptions,
    Client, Collection, Database,
};
use poise::serenity_prelude::GuildId;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::custom_types::{command::Context, mongo_schema::User};
pub async fn receive_client(mongo_uri: &str) -> Result<Client, mongodb::error::Error> {
    let mut client_options = ClientOptions::parse(mongo_uri).await?;
    client_options.app_name = Some("Johnson Bot RS".to_string());

    // Unwrap for now
    Client::with_options(client_options)
}

async fn get_user_collection(db_handle: &Database, guild_id: GuildId) -> Collection<User> {
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

pub async fn get_users<'a>(ctx: &'a Context<'_>) -> Result<Vec<User>, mongodb::error::Error> {
    let g_id = ctx
        .guild_id()
        .expect("Johnson should be called within a guild");

    let user_col = get_user_collection(&ctx.data().johnson_handle, g_id).await;
    get_all_docs(&user_col).await
}
