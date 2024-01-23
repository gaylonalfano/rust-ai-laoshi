//! The `ais` module is designed to be the interface with specific AI services, such as OpenAI.
//!
//! Currently, it is a mono-provider implementation with OpenAI only, but the goal
//! is to be a multi-provider, supporting ollama, lamafile, gemini, etc...
//!
//! Currently, it is mostly designed as an assistant interface, but this might or might not change over time.

// region:       -- Modules
pub mod assistant;
pub mod message;
mod types;

// pub use event::AisEvent;
pub use types::*;

// use crate::event::EventBus;
use crate::{Error, Result};
use async_openai::{config::OpenAIConfig, Client};
// endregion:    -- Modules

// region:       -- Create Async OpenAI Client
const ENV_OPENAI_API_KEY: &str = "OPENAI_API_KEY";

// pub type OaClient = Client<OpenAIConfig>;

pub fn new_openai_client() -> Result<Client<OpenAIConfig>> {
    if std::env::var(ENV_OPENAI_API_KEY).is_ok() {
        Ok(Client::new())
    } else {
        println!("No {ENV_OPENAI_API_KEY} env variable found. Please configure.");
        Err(Error::NoOpenAIApiKeyInEnv)
    }
}

// endregion:    -- Create Async OpenAI Client
