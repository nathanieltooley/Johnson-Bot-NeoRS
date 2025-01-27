pub mod command {
    use crate::serenity::prelude::TypeMapKey;
    use poise::serenity_prelude::Role;
    use reqwest::Client as HttpClient;
    use serde::Deserialize;
    use sqlx::SqlitePool;

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
        pub db_conn: SqlitePool,
        pub kwr: Vec<KeywordResponse>,
        pub http: HttpClient,
    }

    pub struct PartialData {
        pub db_conn: SqlitePool,
        pub kwr: Vec<KeywordResponse>,
        pub welcome_role: Option<Role>,
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
    use serde::{Deserialize, Serialize};
    use sqlx::prelude::FromRow;

    const XP_MULTIPLIER: f64 = 15566f64;
    const XP_TRANSLATION: f64 = 15000f64;
    const EXPO_MULTIPLIER: f64 = 0.0415;

    #[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
    pub struct DbUser {
        pub name: String,
        // user's discord id
        pub id: i64,
        pub vbucks: i64,
        pub exp: i64,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct ServerConfig {
        pub id: i64,
        pub welcome_role_id: Option<i64>,
    }
}
