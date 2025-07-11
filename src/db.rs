use crate::custom_types::command::Context as JContext;
use crate::custom_types::command::SerenityCtxData;
use crate::custom_types::mongo_schema::DbUser;
use crate::custom_types::mongo_schema::ServerConfig;
use crate::utils::math::round_to_100;

use std::f64::consts::E;

use poise::serenity_prelude::RoleId;
use poise::serenity_prelude::User;
use poise::serenity_prelude::{Context, GuildId};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqlitePool;
use tracing::info;
use tracing::instrument;

const XP_MULTIPLIER: f64 = 15566f64;
const XP_TRANSLATION: f64 = 15000f64;
const EXPO_MULTIPLIER: f64 = 0.0415;

#[derive(Debug, Clone)]
pub enum ContextType<'a> {
    Slash(JContext<'a>),
    Classic(&'a Context, GuildId),
}

// Wrapper around a context type (either poise or serenity) for use with database connections.
// Helps abstract away the different ways of getting data from both contexts, mainly the database
// connection.
#[derive(Debug)]
pub struct ContextWrapper<'context> {
    ctx: ContextType<'context>,
}

// 'context is the lifetime of the context passed in
impl<'context> ContextWrapper<'context> {
    async fn get_conn(&self) -> SqlitePool {
        match self.ctx {
            ContextType::Classic(ctx, _) => {
                let read = ctx.data.read().await;
                read.get::<SerenityCtxData>()
                    .expect("Johnson should have a SerenityCtxData set in context")
                    .db_conn
                    .clone()
            }
            ContextType::Slash(ctx) => ctx.data().db_conn.clone(),
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

    // Creates a user, ignores if user already exists
    #[instrument(skip(self))]
    pub async fn create_user(&self, user: &User) -> sqlx::Result<DbUser> {
        let pool = self.get_conn().await;
        let user_id: u64 = user.id.into();
        let user_id = user_id as i64;

        let db_user = sqlx::query_as!(DbUser,
            "insert or ignore into users (name, id, vbucks, exp) values($1, $2, $3, $4) returning *",
            user.name,
            user_id,
            0,
            0
        )
        .fetch_one(&pool)
        .await?;

        info!("Created user {:?}", db_user);

        Ok(db_user)
    }

    // Gets a user from db, creates one if they don't exist
    #[instrument(skip(self))]
    pub async fn get_user(&self, user: &User) -> sqlx::Result<DbUser> {
        let pool = self.get_conn().await;
        let user_id = user_to_id(user);

        let db_user = sqlx::query_as!(DbUser, "SELECT * FROM users WHERE id = $1", user_id)
            .fetch_optional(&pool)
            .await?;

        match db_user {
            Some(x) => {
                info!("Got db user: {:?}", x);
                Ok(x)
            }
            None => self.create_user(user).await,
        }
    }

    pub async fn give_user_money(
        &self,
        user: &User,
        money: i64,
    ) -> sqlx::Result<SqliteQueryResult> {
        let pool = self.get_conn().await;
        let user_id = user_to_id(user);

        // increment the money amount by "money" param
        sqlx::query!(
            "UPDATE users SET vbucks = vbucks + $1 WHERE id = $2",
            money,
            user_id
        )
        .execute(&pool)
        .await
    }

    pub async fn give_user_exp(&self, user: &User, exp: i64) -> sqlx::Result<i64> {
        let pool = self.get_conn().await;
        let user_id = user_to_id(user);

        // increment the money amount by "money" param
        let res = sqlx::query!(
            "UPDATE users SET exp = exp + $1 WHERE id = $2 RETURNING exp",
            exp,
            user_id
        )
        .fetch_one(&pool)
        .await?;

        Ok(res.exp)
    }

    pub async fn user_transaction(
        &self,
        from_user: &User,
        to_user: &User,
        money: i64,
    ) -> sqlx::Result<()> {
        let pool = self.get_conn().await;
        let mut trans = pool.begin().await?;

        let from_user_id = user_to_id(from_user);
        let to_user_id = user_to_id(to_user);

        sqlx::query!(
            "UPDATE users SET vbucks = vbucks - $1 WHERE id = $2",
            money,
            from_user_id
        )
        .execute(&mut *trans)
        .await?;

        sqlx::query!(
            "UPDATE users SET vbucks = vbucks + $1 WHERE id = $2",
            money,
            to_user_id,
        )
        .execute(&mut *trans)
        .await?;

        Ok(())
    }

    pub async fn get_server_conf(&self, guild: GuildId) -> sqlx::Result<ServerConfig> {
        let pool = self.get_conn().await;
        let guild_id = guild_to_id(guild);

        let conf = sqlx::query_as!(
            ServerConfig,
            "SELECT * FROM server_config WHERE id = $1",
            guild_id
        )
        .fetch_one(&pool)
        .await?;

        Ok(conf)
    }

    pub async fn save_welcome_role(&self, guild: GuildId, role: RoleId) -> sqlx::Result<()> {
        let pool = self.get_conn().await;
        let role_id: u64 = role.into();
        let role_id = role_id as i64;
        let guild_id = guild_to_id(guild);

        // upsert
        sqlx::query!(
            "
        INSERT INTO server_config(id, welcome_role_id) 
        VALUES ($1, $2) 
        ON CONFLICT(id) 
        DO 
            UPDATE SET welcome_role_id = $2",
            guild_id,
            role_id
        )
        .execute(&pool)
        .await?;

        Ok(())
    }
}
pub fn level_to_exp(l: i64) -> i64 {
    // I use two "as" "hacks" since Into and TryInto didn't want to work
    // I feel like i64 should be able to go into f64 (at least through TryInto) but whatever
    //
    // Looking into it, this seems like the best way to do it other than
    // implementing an algorithm for it
    round_to_100(
        ((XP_MULTIPLIER * E.powf(EXPO_MULTIPLIER * (l - 1) as f64)) - XP_TRANSLATION).round()
            as i64,
    )
}

pub fn exp_to_level(exp: i64) -> i64 {
    let inside_log: f64 = (exp as f64 + XP_TRANSLATION) / XP_MULTIPLIER;

    ((inside_log.log(E) / EXPO_MULTIPLIER) + 1f64) as i64
}

// weird shit to get around the fact that SQLite doesn't support u64 but discord stores their
// ids as u64
fn user_to_id(user: &User) -> i64 {
    let user_id: u64 = user.id.into();
    user_id as i64
}

fn guild_to_id(guild: GuildId) -> i64 {
    let guild_id: u64 = guild.into();
    guild_id as i64
}
