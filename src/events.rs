use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, Message, Ready};

use tracing::{debug, error, info, instrument};

use crate::custom_types::command::DataMongoClient;
use crate::mongo::{self, validate_user_exp};
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
            // Give money
            if let Err(e) = mongo::give_user_money(
                // Unfortunately this is the best way of getting the client
                // cause of RwLock shenanigans
                ctx.data
                    .read()
                    .await
                    .get::<DataMongoClient>()
                    .expect("Johnson should have value mongo client in context"),
                guild_id,
                message.author.id,
                5,
            )
            .await
            {
                error!("Error occurred during message income: {:?}", e);
            }

            let user_info = mongo::get_user(
                ctx.data.read().await.get::<DataMongoClient>().unwrap(),
                guild_id,
                message.author.id,
            )
            .await
            .unwrap()
            .unwrap();

            // Give EXP
            debug!("{}", mongo::level_to_exp(user_info.level));
            debug!("{}", mongo::exp_to_level(user_info.exp));

            if !validate_user_exp(&user_info) {
                debug!("User {}'s EXP and LEVEL stats don't match!", user_info.name);
            }
        }
    }
}
