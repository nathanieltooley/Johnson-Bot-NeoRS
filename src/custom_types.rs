pub mod command {
    use mongodb::{Client, Database};
    use poise::serenity_prelude::{prelude::TypeMapKey, GuildId};

    #[derive(Debug)]
    // Custom data to send between commands
    pub struct Data {
        pub johnson_handle: Client,
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
        // I would use unsigned ints here but the Integer type
        // in a mongodb server is i64
        pub vbucks: i64,
        pub exp: i64,
        pub level: i64,
        pub slur_count: Option<Document>,
        pub inventory: Option<Vec<Document>>,
        pub stroke_count: Option<u32>,
    }
}
