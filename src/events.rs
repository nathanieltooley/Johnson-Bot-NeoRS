use poise::async_trait;
use poise::serenity_prelude::{Context, EventHandler, Ready};

use tracing::{info, instrument};
pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip_all)]
    async fn ready(&self, _ctx: Context, _ready: Ready) {
        info!("Johnson is running!");
    }
}
