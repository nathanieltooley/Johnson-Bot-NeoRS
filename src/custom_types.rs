pub mod command {
    use mongodb::Client;
    use poise::serenity_prelude::prelude::TypeMapKey;
    use reqwest::Client as HttpClient;
    use rspotify::ClientCredsSpotify;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Clone)]
    #[serde(untagged)]
    pub enum KeywordResponse {
        SingleKW {
            kw: String,
            response: String,
        },
        MultiKW {
            kws: Vec<String>,
            response: String,
        },
        MultiResponse {
            kw: String,
            responses: Vec<String>,
        },
        MultiKWResponse {
            kws: Vec<String>,
            responses: Vec<String>,
        },
        WeightedResponses {
            kw: String,
            responses: Vec<String>,
            weights: Vec<f32>,
        },
        MultiKWWeightedResponses {
            kws: Vec<String>,
            responses: Vec<String>,
            weights: Vec<f32>,
        },
    }

    #[derive(Debug)]
    // Custom data to send between commands
    pub struct Data {
        pub johnson_handle: Client,
        pub kwr: Vec<KeywordResponse>,
        pub http: HttpClient,
        pub spotify_client: ClientCredsSpotify,
    }

    pub struct PartialData {
        pub johnson_handle: Client,
        pub kwr: Vec<KeywordResponse>,
    }

    // Custom error type alias that is an Error that implements Send and Sync (for async stuff)
    pub type Error = Box<dyn std::error::Error + Send + Sync>;
    // Poise context constructed with custom Data and Error types
    pub type Context<'a> = poise::Context<'a, Data, Error>;

    pub struct SerenityCtxData;
    impl TypeMapKey for SerenityCtxData {
        type Value = PartialData;
    }
}

pub mod mongo_schema {
    use mongodb::bson::{DateTime, Document};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct User {
        pub name: String,
        pub discord_id: u64,
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
