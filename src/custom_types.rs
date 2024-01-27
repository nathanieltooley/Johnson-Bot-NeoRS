pub mod command {
    use mongodb::{Client, Database};
    use poise::serenity_prelude::{prelude::TypeMapKey, GuildId};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Debug)]
    // Custom data to send between commands
    pub struct Data {
        // This has to be an Arc since the base Serenity Context is separate from this context
        // but both need to be able to lock each other from the DB
        //
        // Not using an Arc would mean I would most likely have to have two different DB handles
        // which defeats the purpose of avoiding data races
        pub johnson_handle: Arc<Mutex<Database>>,
    }
    // Custom error type alias that is an Error that implements Send and Sync (for async stuff)
    pub type Error = Box<dyn std::error::Error + Send + Sync>;
    // Poise context constructed with custom Data and Error types
    pub type Context<'a> = poise::Context<'a, Data, Error>;

    pub struct DataMongoClient;

    impl TypeMapKey for DataMongoClient {
        type Value = Client;
    }
}

pub mod mongo_schema {
    use mongodb::bson::{DateTime, Document};
    use poise::serenity_prelude::UserId;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct User {
        pub name: String,
        pub discord_id: UserId,
        pub date_created: DateTime,
        pub vbucks: u32,
        pub exp: u32,
        pub level: u16,
        pub slur_count: Option<Document>,
        pub inventory: Option<Vec<Document>>,
        pub stroke_count: Option<u32>,
    }
}
