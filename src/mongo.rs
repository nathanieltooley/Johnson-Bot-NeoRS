use std::f64::consts::E;

use futures::{stream::TryStreamExt, FutureExt};
use mongodb::{
    bson::{doc, oid::ObjectId, Document},
    options::ClientOptions,
    Client, ClientSession, Collection, Database,
};
use poise::serenity_prelude::{GuildId, UserId};
use serde::de::DeserializeOwned;
use tracing::{debug, instrument};

use crate::{custom_types::mongo_schema::User, utils};

static DB_NAME: &str = "Johnson";

static XP_MULTIPLIER: f64 = 15566f64;
static XP_TRANSLATION: f64 = 15000f64;
static EXPO_MULTIPLIER: f64 = 0.0415;

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

async fn update_user_session(
    mongo_client: &'_ Client,
    guild_id: GuildId,
    user_id: UserId,
    update: Document,
) -> Result<(), mongodb::error::Error> {
    let mut session = mongo_client.start_session(None).await?;

    session
        .with_transaction(
            update,
            |session, update| {
                async move {
                    let user_col = get_user_collection(&session.client(), guild_id).await;
                    let user_id_i64: i64 = user_id.into();

                    user_col
                        .update_one_with_session(
                            doc! { "discord_id": user_id_i64 },
                            update.clone(),
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

#[instrument(skip(coll))]
pub async fn give_user_money<'a>(
    coll: &Collection<User>,
    guild_id: GuildId,
    user_id: UserId,
    amount: i64,
) -> Result<(), mongodb::error::Error> {
    debug!("Attempting to give user {}, {} money", user_id, amount);

    let user_id: i64 = user_id.into();

    coll.update_one(
        doc! {"discord_id": user_id},
        doc! {"$inc": doc! {"vbucks": amount}},
        None,
    )
    .await?;

    Ok(())
}

pub async fn set_user_money<'a>(
    coll: &Collection<User>,
    user_id: UserId,
    amount: i64,
) -> Result<(), mongodb::error::Error> {
    debug!("Attemping to set user {}'s money to {}", user_id, amount);

    let user_id: i64 = user_id.into();

    coll.update_one(
        doc! {"discord_id": user_id},
        doc! {"$set": doc! {"vbucks": amount}},
        None,
    )
    .await?;

    Ok(())
}

pub fn level_to_exp(l: i64) -> i64 {
    // I use two as "hacks" since Into and TryInto didn't want to work
    // I feel like i64 should be able to go into f64 (at least through TryInto) but whatever
    //
    // Looking into it, this seems like the best way to do it other than
    // implementing an algorithm for it
    utils::math::round_to_100(
        ((XP_MULTIPLIER * E.powf(EXPO_MULTIPLIER * (l - 1) as f64)) - XP_TRANSLATION).round()
            as i64,
    )
}

pub fn exp_to_level(exp: i64) -> i64 {
    let inside_log: f64 = (exp as f64 + XP_TRANSLATION) / XP_MULTIPLIER;

    ((inside_log.log(E) / EXPO_MULTIPLIER) + 1f64) as i64
}

pub fn validate_user_exp(user: &User) -> bool {
    let level = user.level;
    let exp = user.exp;

    return level_to_exp(level) == exp;
}

// TODO: Write custom transaction that will check for level changes while changing exp
#[instrument(skip(mongo_client))]
pub async fn give_user_exp<'a>(
    mongo_client: &'a Client,
    guild_id: GuildId,
    user_id: UserId,
    amount: i64,
) -> Result<bool, mongodb::error::Error> {
    todo!()
}

pub async fn set_user_exp<'a>(
    mongo_client: &'_ Client,
    guild_id: GuildId,
    user_id: UserId,
    amount: i64,
) -> Result<bool, mongodb::error::Error> {
    todo!()
}
