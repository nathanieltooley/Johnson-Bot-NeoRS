pub mod command {
    use mongodb::Database;
    use tokio::sync::Mutex;

    #[derive(Debug)]
    // Custom data to send between commands
    pub struct Data {
        pub johnson_handle: Mutex<Database>,
    }
    // Custom error type alias that is an Error that implements Send and Sync (for async stuff)
    pub type Error = Box<dyn std::error::Error + Send + Sync>;
    // Poise context constructed with custom Data and Error types
    pub type Context<'a> = poise::Context<'a, Data, Error>;
}

pub mod mongo_schema {
    use mongodb::bson::{DateTime, Document};
    use poise::serenity_prelude::UserId;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct User {
        name: String,
        discord_id: UserId,
        date_created: DateTime,
        vbucks: u32,
        exp: u32,
        level: u16,
        slur_count: Option<Document>,
        inventory: Option<Vec<Document>>,
        stroke_count: Option<u32>,
    }
}
