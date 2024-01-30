use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, Message, Ready};

use tracing::{debug, error, info, instrument};

use crate::mongo::{self, validate_user_exp, ContextWrapper};
pub struct Handler;

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
            if let Err(e) = db_helper.give_user_money(message.author.id, 5).await {
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
            let user_info = get_user_result.unwrap().unwrap();

            // Give EXP
            debug!("{}", mongo::level_to_exp(user_info.level));
            debug!("{}", mongo::exp_to_level(user_info.exp));

            if !validate_user_exp(&user_info) {
                debug!("User {}'s EXP and LEVEL stats don't match!", user_info.name);
            }
        }
    }
}
