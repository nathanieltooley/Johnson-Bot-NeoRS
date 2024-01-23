mod commands;
mod custom_types;
mod logging;

use poise::serenity_prelude::{self as serenity, async_trait, EventHandler, GuildId, Ready};
use poise::Command;

use custom_types::{Data, Error};

use tracing::{info, instrument};

// Custom data to send between commands
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip_all)]
    async fn ready(&self, _ctx: serenity::Context, _ready: Ready) {
        info!("Johnson is running!");
    }
}

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
    ) -> Result<Data, Box<dyn std::error::Error + Sync + Send>> {
        //
        match self {
            // Register the commands globally
            CommandRegistering::Global => {
                poise::builtins::register_globally(ctx, commands).await?;
                Ok(Data {})
            }
            // Register commands for every provided guild
            CommandRegistering::ByGuild(guilds) => {
                for guild in guilds {
                    // Deref and copy guild_id
                    poise::builtins::register_in_guild(ctx, commands, *guild).await?;
                }
                Ok(Data {})
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Init logging
    logging::log_init();

    // Configuration
    let d_token =
        std::env::var("TOKEN").expect("Johnson should be able to find TOKEN environment var");
    let intents = serenity::GatewayIntents::non_privileged();
    let opts = poise::FrameworkOptions {
        commands: vec![commands::basic::ping()],
        ..Default::default()
    };

    // Set register type
    let registering = CommandRegistering::ByGuild(vec![GuildId::new(427299383474782208)]);

    // Build framework
    let framework = poise::Framework::builder()
        .options(opts)
        .setup(|ctx, _ready, framework| {
            // Callback called during setup
            // Requires a pin that holds a future
            Box::pin(async move {
                registering
                    .register(ctx, &framework.options().commands)
                    .await
            })
        })
        .build();

    // Build client
    let mut client = serenity::ClientBuilder::new(d_token, intents)
        .framework(framework)
        .event_handler(Handler)
        .await
        .expect("Client should be built correctly");

    // Start client
    client.start().await.expect("Client error");
}
