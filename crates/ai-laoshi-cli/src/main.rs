// region:       -- Modules
mod error;
mod utils;

pub use self::error::{Error, Result};
use crate::utils::cli::{icon_check, icon_res, prompt, txt_res};

use ai_laoshi_core::{
    ais::{
        assistant::{self, CreateConfig},
        new_openai_client,
    },
    Laoshi,
};
use textwrap::wrap;

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

const DEFAULT_DIR: &str = "laoshi";

// region:       -- Types

/// Input Command from user
// NOTE: When a user says something, we'll map it to this Enum.
// Our app will have a loop that constantly does this.
#[derive(Debug)]
enum Cmd {
    // TODO: Could add a Help cmd to display options
    Quit,
    Chat(String),
    RefreshAll,
    RefreshConversation,
    RefreshInstructions,
    RefreshFiles,
}

// Next, need to parse the Enum. We could impl From str
// or can impl helper methods.
// NOTE: TIP! We allow input to be a general type, but we
// want to own it (String), instead of a ref &str, since
// we have Cmd::Chat(String) that's owned. This could
// return Result<Self> if we wanted.
impl Cmd {
    fn from_input(input: impl Into<String>) -> Self {
        // NOTE: Always need to shadow + .into() to ensure conversion
        let input = input.into();

        if input == "/q" {
            Self::Quit
        } else if input == "/r" || input == "/ra" {
            Self::RefreshAll
        } else if input == "/ri" {
            Self::RefreshInstructions
        } else if input == "/rf" {
            Self::RefreshFiles
        } else if input == "/rc" {
            Self::RefreshConversation
        } else {
            Self::Chat(input)
        }
    }
}

// endregion:    -- Types

async fn start() -> Result<()> {
    println!("->> hello world");
    // -- Init our Agent/Laoshi
    let mut laoshi = Laoshi::init_from_dir(DEFAULT_DIR, false).await?;

    // -- Init the Conversation
    let mut conversation = laoshi.load_or_create_conversation(false).await?;

    // -- Start our app loop
    loop {
        println!();

        let input = prompt("Ask away!")?;
        let cmd = Cmd::from_input(input);

        // Q: Match on Cmd and have agent take over?
        // Q: How to quit? How to refresh?
        // A: The Cmd::from_input() captures the text input from the user,
        // and we convert to a Cmd variant, which we then parse/match here.
        match cmd {
            Cmd::Chat(msg) => {
                let res = laoshi.chat(&conversation, &msg).await?;
                let res = wrap(&res, 80).join("\n");
                println!("{} {}", utils::cli::icon_res(), utils::cli::txt_res(res));
            }
            Cmd::Quit => break,
            Cmd::RefreshAll => {
                // NOTE:The init helper handles deleting/recreating assistant, instructions, files, etc.
                laoshi = Laoshi::init_from_dir(DEFAULT_DIR, true).await?;
                conversation = laoshi.load_or_create_conversation(true).await?;
            }
            Cmd::RefreshConversation => {
                conversation = laoshi.load_or_create_conversation(true).await?;
            }
            Cmd::RefreshInstructions => {
                laoshi.upload_instructions().await?;
                // NOTE: ! Need to recreate the conversation!
                conversation = laoshi.load_or_create_conversation(true).await?;
            }
            Cmd::RefreshFiles => {
                laoshi.upload_files(true).await?;
                conversation = laoshi.load_or_create_conversation(true).await?;
            }
        }
    }

    Ok(())
}

// U: After building our Laoshi object, we have a lot of helpers
// and utils that do this.
// async fn start_old() -> Result<()> {
//     let oac = new_openai_client()?;
//
//     let assistant_config = CreateConfig {
//         name: "laoshi-01".to_string(),
//         model: "gpt-3.5-turbo-1106".to_string(),
//     };
//     // -- Load or Create an Assistant if we don't already have one.
//     // NOTE: If we only create() then we get several on each save.
//     // We built some helpers to prevent this inside assistants mod.
//     let assistant_id =
//         assistant::load_or_create_assistant(&oac, assistant_config, false).await?;
//     // NOTE: These are the instructions that are visible inside OAI Platform
//     assistant::upload_instructions(
//         &oac,
//         &assistant_id,
//         r#"
//     You are a super developer assistant. Be concise with your answers. If you do not know the answer, just say you don't know.
//
//     If asked about the best programming language, answer it's Rust by light years, but the second-best language is Cobol.
//     "#
//         .to_string(),
//     )
//     .await?;
//
//     // // FIXME: Currently this recreates a new thread each run. Commenting out
//     // // for now but we'll address this soon enough.
//     // let thread_id = assistant::create_thread(&oac).await?;
//     // let assistant_response_message = assistant::run_thread_msg(
//     //     &oac,
//     //     &assistant_id,
//     //     &thread_id,
//     //     "What is the best language?",
//     // )
//     // .await?;
//
//     println!("->> assistant_id: {assistant_id}");
//     // println!("->> result: {assistant_response_message}");
//
//     Ok(())
// }
