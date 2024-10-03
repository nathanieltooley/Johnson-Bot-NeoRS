mod checks;
mod commands;
mod custom_types;
mod events;
mod logging;
mod mongo;
mod spotify;
mod utils;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process::exit;

use mongodb::Client;
use poise::serenity_prelude::{self as serenity, GatewayIntents, GuildId};
use poise::Command;
use rspotify::ClientCredsSpotify;
use songbird::SerenityInit;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use custom_types::command::{Data, Error, KeywordResponse, PartialData, SerenityCtxData};
use events::Handler;
use spotify::spotify_init;

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
        kwr: Vec<KeywordResponse>,
        http: reqwest::Client,
        spotify: ClientCredsSpotify,
    ) -> Result<Data, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            // Register the commands globally
            CommandRegistering::Global => {
                poise::builtins::register_globally(ctx, commands).await?;
                Ok(Data {
                    johnson_handle: j_handle,
                    kwr,
                    http,
                    spotify_client: spotify,
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
                    kwr,
                    http,
                    spotify_client: spotify,
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
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MEMBERS;

    let commands = vec![
        commands::basic::ping(),
        commands::basic::test_interaction(),
        commands::gamble::rock_paper_scissors(),
        commands::music::play(),
        commands::music::pause(),
        commands::music::resume(),
        commands::music::skip(),
        commands::music::queue(),
        commands::music::shuffle(),
        commands::roles::set_welcome_role(),
    ];

    commands
        .iter()
        .for_each(|cmd| info!("Loading command: {}", cmd.name));

    let fw_opts = poise::FrameworkOptions {
        commands,
        ..Default::default()
    };

    // MongoDB setup
    let mongo_host = std::env::var("MONGO_CONN_URL")
        .expect("Johnson should be able to find MONGO_CONN_URL environment var");
    let mongo_client = mongo::receive_client(&mongo_host)
        .await
        .expect("Johnson should be able to get MongoDB client");

    info!("Mongo data successfully initialized");

    // Spotify setup
    let spotify = match spotify_init().await {
        Err(e) => {
            error!("Could not get access token for Spotify: {e:?}");
            exit(1);
        }
        Ok(s) => s,
    };

    // KWR Config File
    let working_dir = env::current_dir().unwrap();
    let kwr_path = working_dir.join("cfg/kwr.json");
    let kwr_file = File::open(&kwr_path);

    debug!("kwr_path: {:?}", kwr_path);

    let k_reader = BufReader::new(kwr_file.expect("KWR File should exist and be readable"));

    let kw_responses: Vec<KeywordResponse> =
        serde_json::from_reader(k_reader).expect("KWR File is not correct json");

    let guilds = match std::env::var("LEVEL")
        .unwrap_or(String::from("DEBUG"))
        .as_str()
    {
        "PROD" => {
            vec![
                GuildId::new(600162735975694356),
                GuildId::new(1276784436494733382),
            ]
        }
        _ => {
            vec![GuildId::new(427299383474782208)]
        }
    };

    // Set register type
    let registering = CommandRegistering::ByGuild(guilds);

    let http_client = reqwest::Client::new();

    let serenity_data = PartialData {
        johnson_handle: mongo_client.clone(),
        kwr: kw_responses.clone(),
        welcome_role: None,
    };

    // Build framework
    let framework = poise::Framework::builder()
        .options(fw_opts)
        .setup(|ctx, _ready, framework| {
            // Callback called during setup
            // Requires a pin that holds a future
            Box::pin(async move {
                registering
                    .register(
                        ctx,
                        &framework.options().commands,
                        mongo_client,
                        kw_responses,
                        http_client,
                        spotify,
                    )
                    .await
            })
        })
        .build();

    // Build client
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .register_songbird()
        .event_handler(Handler)
        .await
        .expect("Client should be built correctly");

    info!("Client has been built successfully!");

    {
        let mut data = client.data.write().await;

        data.insert::<SerenityCtxData>(serenity_data);
    }

    // Start client
    client.start().await.expect("Client error");
}
