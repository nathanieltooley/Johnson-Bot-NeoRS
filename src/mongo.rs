use std::f64::consts::E;

use futures::{stream::TryStreamExt, FutureExt};
use mongodb::{
    bson::{doc, Bson, DateTime, Document},
    options::{ClientOptions, FindOneAndReplaceOptions, FindOneAndUpdateOptions, ReturnDocument},
    Client, Collection,
};
use poise::serenity_prelude::{self, Context, GuildId, Role, RoleId, UserId};
use serde::de::DeserializeOwned;
use tracing::{debug, info, instrument};

use crate::custom_types::{command::Context as JContext, mongo_schema::ServerConfig};
use crate::custom_types::{command::SerenityCtxData, mongo_schema::User};
use crate::utils;

static DB_NAME: &str = "johnsondb";

const XP_MULTIPLIER: f64 = 15566f64;
const XP_TRANSLATION: f64 = 15000f64;
const EXPO_MULTIPLIER: f64 = 0.0415;

#[derive(Debug, Clone)]
pub enum ContextType<'a> {
    Slash(JContext<'a>),
    Classic(&'a Context, GuildId),
}

/// Custom error type that encapsulates errors from my code and errors from MongoDB
#[derive(Debug, Clone)]
pub enum MongoError {
    JohnsonError(JohnsonError),
    DriverError(mongodb::error::Error),
}

impl From<JohnsonError> for MongoError {
    fn from(value: JohnsonError) -> Self {
        MongoError::JohnsonError(value)
    }
}

impl From<mongodb::error::Error> for MongoError {
    fn from(value: mongodb::error::Error) -> Self {
        MongoError::DriverError(value)
    }
}

#[derive(Debug, Clone)]
pub enum JohnsonError {
    InsufficientFunds(User, u64),
}

impl std::error::Error for MongoError {}
impl std::error::Error for JohnsonError {}

impl std::fmt::Display for MongoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DriverError(m) => {
                write!(f, "The mongo driver came across an error: {m}")
            }
            Self::JohnsonError(j) => {
                write!(f, "Johnson caused an error during mongo execution: {j}")
            }
        }
    }
}

impl std::fmt::Display for JohnsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientFunds(user, funds) => {
                write!(f, "Johnson attempted to remove {funds} funds from {user:?}")
            }
            #[allow(unreachable_patterns)]
            _ => {
                write!(f, "Unknown error")
            }
        }
    }
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

    pub fn new_classic(ctx: &'context Context, guild_id: GuildId) -> Self {
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

        if (self.get_user(user_id).await?).is_none() {
            create_new_user(&user_col, user_id, user_nick).await?;
            debug!("New user created! {}:{}", user_id, user_nick);
            Ok(())
        } else {
            Ok(())
        }
    }

    pub async fn get_user_safe(
        &self,
        user: &serenity_prelude::User,
    ) -> Result<User, mongodb::error::Error> {
        let user_col = get_user_collection(&self.get_client().await, self.get_guild_id()).await;

        match self.get_user(user.id).await? {
            Some(user) => Ok(user),
            None => {
                let user = create_new_user(&user_col, user.id, &user.name).await?;
                info!("New user created! {}", user.discord_id);
                Ok(user)
            }
        }
    }

    #[instrument(skip_all)]
    pub async fn player_transaction(
        &self,
        user1: &serenity_prelude::User,
        user2: &serenity_prelude::User,
        amount: u64,
    ) -> Result<(), MongoError> {
        let m_user1 = self.get_user_safe(user1).await?;
        let m_user2 = self.get_user_safe(user2).await?;

        if m_user1.vbucks < amount.try_into().unwrap() {
            return Err(JohnsonError::InsufficientFunds(m_user1, amount).into());
        }

        // Remove -amount from user1
        self.give_user_money(
            UserId::new(m_user1.discord_id),
            -(TryInto::<i64>::try_into(amount).unwrap()),
        )
        .await?;

        info!(
            "Removed {amount} from {}:{}",
            m_user1.name, m_user1.discord_id
        );

        // Give user2 amount
        self.give_user_money(UserId::new(m_user2.discord_id), amount.try_into().unwrap())
            .await?;

        info!("Added {amount} to {}:{}", m_user2.name, m_user2.discord_id);

        Ok(())
    }

    #[instrument(skip_all)]
    async fn get_server_conf_collection(&self) -> Result<Collection<ServerConfig>, MongoError> {
        let db = self
            .get_client()
            .await
            .default_database()
            .expect("default db should be set");

        Ok(db.collection("ServerConf"))
    }

    #[instrument(skip_all)]
    async fn get_server_conf(&self) -> Result<ServerConfig, MongoError> {
        let guild_id_raw: i64 = self.get_guild_id().into();
        let server_conf_col = self.get_server_conf_collection().await?;
        let server_conf = server_conf_col
            .find_one(
                doc! {
                    "guild_id": guild_id_raw
                },
                None,
            )
            .await?;

        match server_conf {
            Some(sc) => Ok(sc),
            None => {
                let default_sc = ServerConfig {
                    guild_id: guild_id_raw,
                    welcome_role_id: None,
                };

                Ok(default_sc)
            }
        }
    }

    #[instrument(skip_all)]
    pub async fn save_welcome_role(&self, role: RoleId) -> Result<(), MongoError> {
        let guild_id_raw: i64 = self.get_guild_id().into();
        let server_conf_col = self.get_server_conf_collection().await?;

        let mut server_conf = self.get_server_conf().await?;
        server_conf.welcome_role_id = Some(role.into());

        let _ = server_conf_col
            .find_one_and_replace(
                doc! {
                    "guild_id": guild_id_raw
                },
                server_conf,
                Some(FindOneAndReplaceOptions::builder().upsert(true).build()),
            )
            .await?;

        info!(
            "Set the welcome role of guild {} to id {}",
            guild_id_raw, role
        );

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn get_welcome_role(&self) -> Result<Option<RoleId>, MongoError> {
        let server_conf = self.get_server_conf().await?;

        Ok(server_conf.welcome_role_id.map(RoleId::new))
    }
}

#[instrument]
pub async fn receive_client(mongo_uri: &str) -> Result<Client, mongodb::error::Error> {
    let mut client_options = ClientOptions::parse(mongo_uri).await?;
    client_options.app_name = Some("Johnson Bot RS".to_string());
    client_options.default_database = Some(DB_NAME.to_owned());

    Client::with_options(client_options)
}

pub async fn get_user_collection(mongo_client: &Client, guild_id: GuildId) -> Collection<User> {
    mongo_client
        .default_database()
        .expect("default database should be specified")
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

pub async fn get_users(
    mongo_client: &Client,
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
        .find_one(doc! {"discord_id": Into::<i64>::into(user_id)}, None)
        .await
}

pub async fn get_user_bsonid(
    user_col: &Collection<User>,
    _id: Bson,
) -> Result<Option<User>, mongodb::error::Error> {
    debug!("Attempting to get user with _id: {_id}");
    user_col
        .find_one(
            doc! {
                "_id": _id
            },
            None,
        )
        .await
}

pub async fn get_user_from_col(
    user_col: &Collection<User>,
    user_id: UserId,
) -> Result<Option<User>, mongodb::error::Error> {
    user_col
        .find_one(doc! {"discord_id": Into::<i64>::into(user_id)}, None)
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
            // Make it so that it returns the user after it is updated
            Some(
                FindOneAndUpdateOptions::builder()
                    .return_document(ReturnDocument::After)
                    .build(),
            ),
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
    // I use two "as" "hacks" since Into and TryInto didn't want to work
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

    exp >= min_exp && exp <= next_exp
}

#[instrument(skip_all)]
pub async fn create_new_user(
    user_col: &Collection<User>,
    user_id: UserId,
    user_nick: &str,
) -> Result<User, mongodb::error::Error> {
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

    let _id = user_col.insert_one(def_user, None).await?;
    let new_user = get_user_bsonid(user_col, _id.inserted_id).await?.unwrap();

    info!("Created new user: {:?}", new_user);

    Ok(new_user)
}
