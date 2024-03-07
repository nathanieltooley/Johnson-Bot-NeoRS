use std::f64::consts::E;

use futures::{stream::TryStreamExt, FutureExt};
use mongodb::{
    bson::{doc, DateTime, Document},
    options::ClientOptions,
    Client, Collection,
};
use poise::serenity_prelude::{Context, GuildId, UserId};
use serde::de::DeserializeOwned;
use tracing::{debug, instrument};

use crate::custom_types::command::Context as JContext;
use crate::custom_types::{command::SerenityCtxData, mongo_schema::User};
use crate::utils;

static DB_NAME: &str = "Johnson";

const XP_MULTIPLIER: f64 = 15566f64;
const XP_TRANSLATION: f64 = 15000f64;
const EXPO_MULTIPLIER: f64 = 0.0415;

pub enum ContextType<'a> {
    Slash(JContext<'a>),
    Classic(&'a Context, GuildId),
}

/// This struct is to be a wrapper around both types of context
/// so that they may access MongoDB helpers more efficiently.
///
/// This is mainly for the older Serenity context which requires a lot of code to
/// access context data.
pub struct ContextWrapper<'context> {
    ctx: ContextType<'context>,
}

// 'context is the lifetime of the context passed in
impl<'context> ContextWrapper<'context> {
    async fn get_client(&self) -> Client {
        match self.ctx {
            ContextType::Classic(ctx, _) => {
                let read = ctx.data.read().await;
                read.get::<SerenityCtxData>()
                    .expect("Johnson should have a SerenityCtxData set in context")
                    .johnson_handle
                    .clone()
            }
            ContextType::Slash(ctx) => ctx.data().johnson_handle.clone(),
        }
    }

    fn get_guild_id(&self) -> GuildId {
        match self.ctx {
            ContextType::Slash(ctx) => ctx
                .guild_id()
                .expect("This method should never be called inside of DMs"),
            ContextType::Classic(_, gid) => gid,
        }
    }

    pub fn new_classic<'a>(ctx: &'context Context, guild_id: GuildId) -> Self {
        ContextWrapper {
            ctx: ContextType::Classic(ctx, guild_id),
        }
    }

    pub fn new_slash(ctx: JContext<'context>) -> Self {
        ContextWrapper {
            ctx: ContextType::Slash(ctx),
        }
    }

    pub async fn get_user(&self, user_id: UserId) -> Result<Option<User>, mongodb::error::Error> {
        get_user(&self.get_client().await, self.get_guild_id(), user_id).await
    }

    pub async fn give_user_money(
        &self,
        user_id: UserId,
        amount: i64,
    ) -> Result<(), mongodb::error::Error> {
        let guild_id = self.get_guild_id();
        let user_col = get_user_collection(&self.get_client().await, guild_id).await;

        give_user_money(&user_col, guild_id, user_id, amount).await
    }

    #[instrument(skip(self))]
    pub async fn give_user_exp(
        &self,
        user_id: UserId,
        amount: i64,
    ) -> Result<Option<i64>, mongodb::error::Error> {
        let client = self.get_client().await;
        let user_col = get_user_collection(&client, self.get_guild_id()).await;
        let mut session = client.start_session(None).await?;

        // we do all of this in a transaction because we have to check the exp
        // and level multiple times
        session
            .with_transaction(
                &user_col,
                |session, coll| {
                    async move {
                        let user_info = self.get_user(user_id).await?;
                        let mut user = user_info.unwrap();

                        let old_exp = user.exp;
                        let old_level = user.level;

                        let user_id64: i64 = user_id.into();

                        // If this person had an exp based on the old algorithm,
                        // reset their exp according to their level
                        if !validate_user_exp(&user) {
                            debug!("User {}'s EXP and LEVEL stats don't match!", user.name);
                            user.exp = level_to_exp(user.level);
                            debug!("Reset user exp to: {}", user.exp);
                        }

                        // calculate their new exp and level values before updating them
                        let new_xp = user.exp + amount;
                        let level = exp_to_level(new_xp);

                        coll.update_one_with_session(
                            doc! {"discord_id": user_id64},
                            // We could remove the $inc call and just do two sets but not that important
                            doc! {"$set": doc! {"level": level, "exp": new_xp}},
                            None,
                            session,
                        )
                        .await?;

                        debug!("User exp changed from {} -> {}", old_exp, new_xp);

                        // If the user's level changed because of this function
                        // return the new level
                        if level != old_level {
                            Ok(Some(level))
                        } else {
                            Ok(None)
                        }
                    }
                    .boxed()
                },
                None,
            )
            .await
    }

    pub async fn create_user_if_none(
        &self,
        user_id: UserId,
        user_nick: &str,
    ) -> Result<(), mongodb::error::Error> {
        let user_col = get_user_collection(&self.get_client().await, self.get_guild_id()).await;

        if let None = self.get_user(user_id).await? {
            create_new_user(&user_col, user_id, user_nick).await?;
            debug!("New user created! {}:{}", user_id, user_nick);
            Ok(())
        } else {
            Ok(())
        }
    }
}

#[instrument]
pub async fn receive_client(mongo_uri: &str) -> Result<Client, mongodb::error::Error> {
    let mut client_options = ClientOptions::parse(mongo_uri).await?;
    client_options.app_name = Some("Johnson Bot RS".to_string());

    Client::with_options(client_options)
}

pub async fn get_user_collection(mongo_client: &Client, guild_id: GuildId) -> Collection<User> {
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
    let user_id: i64 = user_id.into();

    let user_info = coll
        .find_one_and_update(
            doc! {"discord_id": user_id},
            doc! {"$inc": doc! {"vbucks": amount}},
            None,
        )
        .await?;

    let user = user_info.expect("User should have been created before calling method");

    debug!("User money is now {}", user.vbucks);

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
    let next_level = level + 1;

    let min_exp = level_to_exp(level);
    let exp = user.exp;
    let next_exp = level_to_exp(next_level);

    return exp >= min_exp && exp <= next_exp;
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

pub async fn create_new_user(
    user_col: &Collection<User>,
    user_id: UserId,
    user_nick: &str,
) -> Result<(), mongodb::error::Error> {
    let def_user = User {
        name: user_nick.to_string(),
        discord_id: user_id.into(),
        date_created: DateTime::now(),
        vbucks: 0,
        exp: 0,
        level: 0,
        slur_count: None,
        inventory: None,
        stroke_count: None,
    };

    user_col.insert_one(def_user, None).await?;
    Ok(())
}
