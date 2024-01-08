// region:       -- Modules
mod error;

pub use self::error::{Error, Result};

use ai_laoshi_core::ais::{
    assistant::{self, CreateConfig},
    new_openai_client,
};

// endregion:    -- Modules

#[tokio::main]
async fn main() {
    // NOTE: Preference is to keep main() small, and then
    // use other helpers to run the loop, etc.
    println!();

    match start().await {
        Ok(_) => println!("\nBye!\n"),
        Err(e) => println!("\nError: {}\n", e),
    }
}

async fn start() -> Result<()> {
    let oac = new_openai_client()?;

    let assistant_config = CreateConfig {
        name: "laoshi-01".to_string(),
        model: "gpt-3.5-turbo-1106".to_string(),
    };
    // -- Load or Create an Assistant if we don't already have one.
    // NOTE: If we only create() then we get several on each save.
    // We built some helpers to prevent this inside assistants mod.
    let assistant_id =
        assistant::load_or_create_assistant(&oac, assistant_config, false).await?;

    println!("->> assistant_id: {assistant_id}");

    Ok(())
}
