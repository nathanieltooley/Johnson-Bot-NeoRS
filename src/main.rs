mod checks;
mod commands;
mod custom_types;
mod events;
mod logging;
mod mongo;
mod utils;

use mongodb::Client;
use poise::serenity_prelude::{self as serenity, GatewayIntents, GuildId};
use poise::Command;

use custom_types::command::{Data, DataMongoClient, Error};
use events::Handler;
use tracing::info;

#[allow(dead_code)]
enum CommandRegistering {
    Global,
    ByGuild(Vec<GuildId>),
}

impl<'a> CommandRegistering {
    async fn register(
        &self,
        ctx: &serenity::Context,
        commands: &[Command<Data, Error>],
        j_handle: Client,
    ) -> Result<Data, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            // Register the commands globally
            CommandRegistering::Global => {
                poise::builtins::register_globally(ctx, commands).await?;
                Ok(Data {
                    johnson_handle: j_handle,
                })
            }
            // Register commands for every provided guild
            CommandRegistering::ByGuild(guilds) => {
                for guild in guilds {
                    // Deref and copy guild_id
                    poise::builtins::register_in_guild(ctx, commands, *guild).await?;
                }
                Ok(Data {
                    johnson_handle: j_handle,
                })
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Init logging
    logging::log_init();

    // Configuration
    let token =
        std::env::var("TOKEN").expect("Johnson should be able to find TOKEN environment var");
    let intents = serenity::GatewayIntents::non_privileged()
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let opts = poise::FrameworkOptions {
        commands: vec![commands::basic::ping()],
        ..Default::default()
    };

    // MongoDB setup
    let mongo_host = std::env::var("DISCORD_HOST")
        .expect("Johnson should be able to find MONGO_HOST environment var");
    let mongo_client = mongo::receive_client(&mongo_host)
        .await
        .expect("Johnson should be able to get MongoDB client");

    info!("Mongo data successfully initialized");

    // Set register type
    let registering = CommandRegistering::ByGuild(vec![GuildId::new(427299383474782208)]);

    let context_client = mongo_client.clone();
    // Build framework
    let framework = poise::Framework::builder()
        .options(opts)
        .setup(|ctx, _ready, framework| {
            // Callback called during setup
            // Requires a pin that holds a future
            Box::pin(async move {
                registering
                    .register(ctx, &framework.options().commands, context_client)
                    .await
            })
        })
        .build();

    // Build client
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .event_handler(Handler)
        .await
        .expect("Client should be built correctly");

    info!("Client has been built successfully!");

    let mut data = client.data.write().await;
    data.insert::<DataMongoClient>(mongo_client);

    // Drop the lock and the borrow of client
    drop(data);

    // Start client
    client.start().await.expect("Client error");
}
