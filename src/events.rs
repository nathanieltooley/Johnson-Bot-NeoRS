use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, Message, Ready};

use tracing::{debug, error, info, instrument};

use crate::custom_types::command::JohnsonDBHandle;
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
            // Get read lock on data in Context
            let ctx_data = ctx.data.read().await;

            // Get the handle Mutex inside of the Data map and then get the lock for it
            let handle = ctx_data
                .get::<JohnsonDBHandle>()
                .expect("Johnson expected there to be a db handle inside context data")
                .lock()
                .await;

            // Attempt to get the users from the DB
            let users = mongo::get_users(guild_id, handle).await;

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
