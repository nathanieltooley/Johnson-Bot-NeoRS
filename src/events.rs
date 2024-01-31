use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, Message, Ready};

use rand::Rng;
use tracing::{debug, error, info, instrument};

use crate::mongo::ContextWrapper;
pub struct Handler;

const MONEY_MIN: i64 = 5;
const MONEY_MAX: i64 = 20;

const EXP_PER_MESSAGE: i64 = 200;

// Extract out the code for this logic since ThreadRNG is not thread safe
fn money_rand() -> i64 {
    let mut rng = rand::thread_rng();

    rng.gen_range(MONEY_MIN..MONEY_MAX)
}

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip_all)]
    async fn ready(&self, _ctx: Context, _ready: Ready) {
        info!("Johnson is running!");
    }

    #[instrument(skip_all)]
    async fn message(&self, ctx: Context, message: Message) {
        if let Some(guild_id) = message.guild_id {
            let db_helper = ContextWrapper::new_classic(&ctx, guild_id);

            // Give money
            if let Err(e) = db_helper
                .give_user_money(message.author.id, money_rand())
                .await
            {
                error!("Error occurred during message income: {:?}", e);
            }

            // TODO: Add a check at the beginning of this function that creates a users in the DB
            // if they don't have an entry
            let get_user_result = db_helper.get_user(message.author.id).await;

            if let Err(e) = get_user_result {
                error!("Error occured when trying to get user info: {:?}", e);
                return;
            }

            // Panic:
            // This would only panic if we did not have a user in the DB
            // since we return early if there is an error with Mongo
            let user_info = get_user_result
                .expect("Error should've been handled already")
                .expect("User should have been created already");

            let actual_level = user_info.level;

            match db_helper
                .give_user_exp(message.author.id, EXP_PER_MESSAGE)
                .await
            {
                Ok(res) => {
                    if let Some(new_level) = res {
                        debug!(
                            "User {}'s level has changed from {} to {}!",
                            user_info.name, actual_level, new_level
                        );
                    }
                }
                Err(e) => {
                    error!("Error when attempting to give user exp: {:?}", e);
                }
            }
        }
    }
}
