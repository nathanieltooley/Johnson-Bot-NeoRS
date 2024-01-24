#[derive(Debug)]
// Custom data to send between commands
pub struct Data {}
// Custom error type alias that is an Error that implements Send and Sync (for async stuff)
pub type Error = Box<dyn std::error::Error + Send + Sync>;
// Poise context constructed with custom Data and Error types
pub type Context<'a> = poise::Context<'a, Data, Error>;
