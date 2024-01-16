//! The `laoshi` module handles everything related to the laoshi construct.
//!
//! A laoshi is an abstraction above an assistant, offering high-level functionalities
//! for on-device applications (CLI or UI-APP).
//!
//! Buddies are scoped to on-device use because they're not designed to handle multi-user requests,
//! but rather tailored for single-user interactions.
//!
//! Single-user requests don't imply a sequential-request design; they might support request concurrency under certain conditions.
//! However, due to the nature of AI Conversation/Thread contexts, most requests for a single "laoshi" need to be sequential.
//!
//! Currently, the API doesn't enforce a "sequential" scheme, but it will eventually, while remaining transparent to the API user.

// region:       -- Modules

mod config;

use crate::{
    ais::{
        self,
        assistant::{self, AssistantId, ThreadId},
    },
    laoshi::config::Config,
    utils::files::bundle_to_file,
    Error, Result,
};

// use rust-ai-laoshi::ai-laoshi-cli::utils::cli::icon_check();

use async_openai::{config::OpenAIConfig, Client};
use derive_more::{Deref, From};
use serde::{Deserialize, Serialize};
use simple_fs::{ensure_dir, list_files, load_toml, read_to_string, SPath};
use std::path::{Path, PathBuf};

// endregion:    -- Modules

const LAOSHI_TOML: &str = "laoshi.toml";

// NOTE: TIP! When new to Rust and making structs, 90% of time
// make sure to OWN the data! E.g., PathBuf (owned) instead of Path (ref).
// REF: https://youtu.be/PHbCmIckV20?t=4346
#[derive(Debug)]
pub struct Laoshi {
    dir: PathBuf,
    oac: Client<OpenAIConfig>,
    assistant_id: AssistantId,
    config: Config,
}

// NOTE: TIP! It's better to wrap types (eg. String) with our custom types,
// i.e., ThreadId(String) rather than dealing with a bunch of Strings getting
// passed around. We want to convert from a ThreadId(String) into a Conversation obj.
// NOTE: Deref allows us to go from a Conversation into a ThreadId
// NOTE: We add both Serialize/Deserialize since we'll store the
// conversation in a laoshi.json file, so we don't have to create
// a new ThreadId each time we run/use cargo watch.
#[derive(Debug, From, Deref, Serialize, Deserialize)]
pub struct Conversation {
    thread_id: ThreadId,
}

impl Laoshi {
    // -- Constructor functions
    // NOTE: This is where we use all our helpers with assistants, threads, ixs, etc.
    // to build our custom Agent/"Buddy" abstraction obj.
    // REF: https://youtu.be/PHbCmIckV20?t=6816
    pub async fn init_from_dir(
        dir: impl AsRef<Path>,
        recreate_assistant: bool, // For assistant::load_or_create_assistant()
    ) -> Result<Self> {
        let dir = dir.as_ref();

        // -- Load from the directory
        let config: Config = load_toml(dir.join(LAOSHI_TOML))?;

        // -- Get or create our OAI Assistant
        let oac = ais::new_openai_client()?;
        let assistant_id = ais::assistant::load_or_create_assistant(
            &oac,
            // Q: Why does &config.into() convert into '&_'
            // A: Wrap &config with () works...
            (&config).into(),
            recreate_assistant,
        )
        .await?;

        // -- Create the Laoshi agent
        let laoshi = Laoshi {
            dir: dir.to_path_buf(),
            oac,
            assistant_id,
            config,
        };

        // -- Upload instructions
        laoshi.upload_instructions().await?;

        // -- Upload files
        // TODO:
        // laoshi.upload_files().await?;

        todo!()
    }

    // -- Public functions
    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub async fn upload_instructions(&self) -> Result<bool> {
        let file = &self.dir.join(&self.config.instructions_file);
        if file.exists() {
            // -- Upload ix and return 'true'
            let ix_content = read_to_string(file)?;
            // Q: How to convert Result<()> into Result<bool>?
            assistant::upload_instructions(
                &self.oac,
                &self.assistant_id,
                ix_content,
            )
            .await?;

            println!(
                "{} Instructions uploaded",
                // FIXME:
                // Q: How to use outside crate ai-laoshi-cli?
                // Can't access ai_laoshi_cli::utils::cli::icon_check()
                // ai_laoshi_cli::utils::cli::icon_check()
                "✓"
            );

            Ok(true)
        } else {
            // -- Return Ok(false) since we didn't have any ixs
            Ok(false)
        }

        // match assistant::upload_instructions(
        //     &self.oac,
        //     &self.assistant_id,
        //     ix_content,
        // )
        // .await?
        // {
        //     Ok(_) => Ok(true),
        //     Err(_) => Err(false),
        // }
    }

    // -- Private functions
    /// Where we store conversations, data, bundles, instructions
    fn data_dir(&self) -> Result<PathBuf> {
        let data_dir = self.dir.join(".laoshi"); // laoshi/.laoshi/files/conv.json
        ensure_dir(&data_dir)?;
        Ok(data_dir)
    }

    /// Where we store file bundles
    fn data_files_dir(&self) -> Result<PathBuf> {
        let dir = self.data_dir()?.join("files");
        // NOTE: TIP! We could write ensure_dir() helper here, but better
        // is to create and use a utils module for these.
        ensure_dir(&dir)?;
        Ok(dir)
    }
}
