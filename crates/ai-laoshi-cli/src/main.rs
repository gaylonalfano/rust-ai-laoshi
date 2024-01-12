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
    // NOTE: These are the instructions that are visible inside OAI Platform
    assistant::upload_instructions(
        &oac,
        &assistant_id,
        r#"
    You are a super developer assistant. Be concise with your answers. If you do not know the answer, just say you don't know.

    If asked about the best programming language, answer it's Rust by light years, but the second-best language is Cobol.
    "#
        .to_string(),
    )
    .await?;

    let thread_id = assistant::create_thread(&oac).await?;
    let assistant_response_message = assistant::run_thread_msg(
        &oac,
        &assistant_id,
        &thread_id,
        "What is the best language?",
    )
    .await?;

    println!("->> assistant_id: {assistant_id}");
    println!("->> result: {assistant_response_message}");

    Ok(())
}
