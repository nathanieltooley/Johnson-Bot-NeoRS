use futures::{stream::TryStreamExt, FutureExt};
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::ClientOptions,
    Client, ClientSession, Collection, Database,
};
use poise::serenity_prelude::{GuildId, UserId};
use serde::de::DeserializeOwned;
use tracing::{debug, instrument};

use crate::custom_types::mongo_schema::User;

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

#[instrument(skip_all, fields(guild_id, user_id))]
pub async fn get_user(
    mongo_client: &'_ Client,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<Option<User>, mongodb::error::Error> {
    debug!("Attempting to get user: {}", user_id);
    let user_col = get_user_collection(mongo_client, guild_id).await;
    user_col
        .find_one(
            doc! {"discord_id": TryInto::<i64>::try_into(user_id).unwrap()},
            None,
        )
        .await
}

pub async fn get_user_from_col(
    user_col: &Collection<User>,
    user_id: UserId,
) -> Result<Option<User>, mongodb::error::Error> {
    user_col
        .find_one(
            doc! {"discord_id": TryInto::<i64>::try_into(user_id).unwrap()},
            None,
        )
        .await
}

pub async fn update_user<'a>(
    mongo_client: &'a Client,
    guild_id: GuildId,
    filter: Document,
    update: Document,
) -> Result<(), mongodb::error::Error> {
    let mut session = mongo_client.start_session(None).await?;

    session
        .with_transaction(
            (&filter, &update),
            |session, (filter, update)| {
                async move {
                    let user_col = get_user_collection(&session.client(), guild_id).await;

                    user_col
                        .update_one_with_session(filter.clone(), update.clone(), None, session)
                        .await
                }
                .boxed()
            },
            None,
        )
        .await?;

    Ok(())
}

#[instrument(skip(mongo_client))]
pub async fn give_user_money<'a>(
    mongo_client: &'a Client,
    guild_id: GuildId,
    user_id: UserId,
    amount: i32,
) -> Result<(), mongodb::error::Error> {
    let mut session = mongo_client.start_session(None).await?;

    session
        .with_transaction(
            (),
            |session, _| {
                async move {
                    debug!("Attempting to give user {}, {} money", user_id, amount);
                    let user_col = get_user_collection(&session.client(), guild_id).await;

                    user_col
                        .update_one_with_session(
                            doc! { "discord_id": TryInto::<i64>::try_into(user_id).unwrap() },
                            doc! {"$inc": doc! {"vbucks": amount}},
                            None,
                            session,
                        )
                        .await
                }
                .boxed()
            },
            None,
        )
        .await?;

    Ok(())
}
