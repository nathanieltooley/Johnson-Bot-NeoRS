use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, Message, Ready};

use tracing::{debug, error, info, instrument};

use crate::custom_types::command::DataMongoClient;
use crate::mongo;
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
            let users = mongo::get_users(
                ctx.data.read().await.get::<DataMongoClient>().unwrap(),
                guild_id,
            )
            .await;

            // Print out the values if it succeeded
            match users {
                Ok(users) => {
                    for user in users {
                        debug!("User: {:?}", user);
                    }
                }
                Err(e) => {
                    error!("{}", e);
                }
            }
        }
    }
}
