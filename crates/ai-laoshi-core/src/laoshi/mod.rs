//! The `laoshi` module handles everything related to the laoshi construct.
//!
//! A laoshi is an abstraction above an assistant, offering high-level functionalities
//! for on-device applications (CLI or UI-APP).
//!
//! Laoshis/Agents are scoped to on-device use because they're not designed to handle multi-user requests,
//! but rather tailored for single-user interactions.
//!
//! Single-user requests don't imply a sequential-request design; they might support request concurrency under certain conditions.
//! However, due to the nature of AI Conversation/Thread contexts, most requests for a single "laoshi" need to be sequential.
//!
//! Currently, the API doesn't enforce a "sequential" scheme, but it will eventually, while remaining transparent to the API user.

// region:       -- Modules

mod config;

use crate::ais::assistant::{self, load_or_create_assistant, upload_file_by_name};
use crate::ais::{new_openai_client, AssistantId, ThreadId};
use crate::laoshi::config::Config;
use crate::utils::files::bundle_to_file;
use crate::{Error, Result};

use async_openai::{config::OpenAIConfig, Client};
use derive_more::{Deref, From};
use serde::{Deserialize, Serialize};
use simple_fs::{
    ensure_dir, list_files, load_toml, read_to_string, save_json, SPath,
};
use std::fs;
use std::path::{Path, PathBuf};

// endregion:    -- Modules
// NOTE: ! EVERYTHING file system related depends on where
// this TOML file is located locally and its configuration!
// This affects the Laoshi.dir PathBuf and uploads/deletions,etc.
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
        let dir = dir.as_ref(); // DEFAULT_DIR = "laoshi"

        // -- Load from the directory
        let config: Config = load_toml(dir.join(LAOSHI_TOML))?; // laoshi/laoshi.toml

        // -- Get or create our OAI Assistant
        let oac = new_openai_client()?;
        let assistant_id = load_or_create_assistant(
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
        // NOTE: Not forcing an upload, since we will upload the bundle file
        // if it's not present.
        laoshi.upload_files(false).await?;

        Ok(laoshi)
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

            Ok(true)
        } else {
            // -- Return Ok(false) since we didn't have any ixs
            Ok(false)
        }
    }

    // NOTE: Conversations will be serialized and stored in a conv.json
    // file within the data_dir (agent/.agent/conv.json). This way
    // we can persist the conversation in a way between sessions.
    pub async fn load_or_create_conversation(
        &self,
        recreate: bool,
    ) -> Result<Conversation> {
        let conversation_file = self.data_dir()?.join("conv.json");

        // -- Delete if recreate and exists
        if recreate && conversation_file.exists() {
            fs::remove_file(&conversation_file)?;
            println!("Conversation file deleted. Creating new file...");
        }

        // -- Previous conversation exists, let's load
        // WARN: Q: What's the mental model? How to get Thread?
        // The JSON will be something like: "thread_id": "thread_abc123"
        // - Laoshi has Assistant
        // - Assistant runs in a Thread and responds to user
        // A: Yep, we parse the JSON and use our assistant::get_thread()
        // helper!
        let conversation = if let Ok(conversation) =
            simple_fs::load_json::<Conversation>(&conversation_file)
        {
            // -- Successfully loaded and converted to Conversation; Get Conversation.thread_id
            assistant::get_thread(&self.oac, &conversation.thread_id)
                .await
                .map_err(|_| {
                    Error::CannotFindThreadIdForConv(conversation.to_string())
                })?;
            // println!("{} conversation loaded", icon_check());
            println!("Conversation loaded");
            conversation
        } else {
            // -- No prior Conversation or conv.json file found; Create new Conversation
            let thread_id = assistant::create_thread(&self.oac).await?;
            println!("Conversation created");
            // Convert ThreadId into a Conversation struct
            // Q: How does this work/convert? Deref? From trait?
            let conversation = thread_id.into();
            // Save/create the conv.json file
            save_json(&conversation_file, &conversation)?;
            conversation
        };

        Ok(conversation)
    }

    pub async fn chat(&self, conv: &Conversation, msg: &str) -> Result<String> {
        // Q: What's the mental model here? We return the model response in String?
        // A: That's exactly what our assistant::run_thread_msg() does!
        // NOTE: Assistants don't know about our custom Conversation, only ThreadId
        let res = assistant::run_thread_msg(
            &self.oac,
            &self.assistant_id,
            &conv.thread_id,
            msg,
        )
        .await?;

        Ok(res)
    }

    pub async fn upload_files(&self, recreate: bool) -> Result<u32> {
        let mut num_uploaded = 0;

        // -- Get the laoshi/files directory
        let data_files_dir = self.data_files_dir()?; // laoshi/files directory

        // -- Clean out old/obsolete files from laoshi/files directory
        let excluded_element = format!("*{}*", &self.assistant_id);
        for file in list_files(
            &data_files_dir,
            Some(&["*.rs", "*.md"]),
            Some(&[&excluded_element]),
        )? {
            // Safeguard
            if !file.to_str().contains(".laoshi") {
                return Err(Error::ShouldNotDeleteLocalFile(file.to_string()));
            }
            // Delete old file
            fs::remove_file(&file)?;
        }

        // -- Generate and upload the laoshi/files bundles

        // -- Get the FileBundle from config.file_bundles (Vec<FileBundle),
        // -- Loop over Vec<FileBundle> and use our new helper upload_file_by_name()
        // NOTE: We use .iter() because we want a reference &FileBundle
        // Lots of details worth watching again and again:
        // REF: https://youtu.be/PHbCmIckV20?t=9899
        // Q: Where is self.config.file_bundles stored locally?
        // - self.config ->> Config
        // - self.config.file_bundles ->> Vec<FileBundle>
        // - FileBundle ->> FileBundle {name, src_dir, src_glob, dst_ext}
        // - FileBundle.src_dir ->> "files" or "../crates" from laoshi.toml
        // Q: self.dir.join(&bundle.src_dir) ->> "laoshi/files", right?
        // A: self.dir ->> the dir of where laoshi.toml is stored ie "laoshi",
        // so, yes, self.dir.join(&bundle.src_dir) ->> "laoshi/files"
        for bundle in self.config.file_bundles.iter() {
            // Get the specific bundle's src_dir (e.g, "laoshi/files", "crates")
            //
            let src_dir = self.dir.join(&bundle.src_dir);

            // Check that we have an existing dir
            if src_dir.is_dir() {
                // NOTE: Get our src_globs (e.g., ["**/*.rs] or ["*.md"]) as a Vec<&str>
                // so we can pass as a slice of ref of String (&[&str]) needed for list_files() below.
                let src_globs: Vec<&str> =
                // Q: How to use map() to return &str from String? Any difference
                // between these approaches?
                    // bundle.src_globs.iter().map(|g| g.as_ref()).collect();
                bundle.src_globs.iter().map(AsRef::as_ref).collect();
                // bundle.src_globs.iter().map(|g| AsRef::as_ref(g)).collect();

                let files = list_files(&src_dir, Some(&src_globs), None)?;

                if !files.is_empty() {
                    // Compute the bundle file name
                    let bundle_file_name = format!(
                        "{}-{}-bundle-{}.{}",
                        self.name(),        // "laoshi-01"
                        bundle.bundle_name, // "knowledge"
                        self.assistant_id,  // "???"
                        bundle.dst_ext,     // "md"
                    );
                    // Build full path file name: laoshi-01-knowledge-bundle-???.md
                    let bundle_file = self.data_files_dir()?.join(bundle_file_name);
                    // NOTE: Here bundle_file is an SPath because the file does not exist
                    // (SFile construction does an is_file() check by contract)
                    let bundle_file = SPath::try_from(bundle_file)?;
                    // let bundle_file = SPath::from_path(bundle_file)?;

                    // If the file doesn't exist, force a re-upload
                    // NOTE: TIP! You can use the presence of a file as state sometimes
                    let force_reupload = recreate || !bundle_file.path().exists();

                    // Rebundle no matter if it already exists (while still developing)
                    // Q: How to convert from PathBuf ->> SPath
                    bundle_to_file(files, &bundle_file)?;

                    // Upload and attach to Assistant
                    let (_, has_uploaded) = assistant::upload_file_by_name(
                        &self.oac,
                        &self.assistant_id,
                        &bundle_file,
                        force_reupload,
                    )
                    .await?;

                    // Update our total upload count
                    if has_uploaded {
                        num_uploaded += 1;
                    }
                }
            }
        }
        // -- Return u32 for number of files uploaded
        Ok(num_uploaded)
    }

    // -- Private functions
    /// Where we store conversations, data, bundles, instructions
    fn data_dir(&self) -> Result<PathBuf> {
        let data_dir = self.dir.join(".laoshi"); // laoshi/.laoshi
        ensure_dir(&data_dir)?;
        Ok(data_dir)
    }

    /// Where we store file bundles
    fn data_files_dir(&self) -> Result<PathBuf> {
        // laoshi/file directory
        let dir = self.data_dir()?.join("files");
        // NOTE: TIP! We could write ensure_dir() helper here, but better
        // is to create and use a utils module for these.
        ensure_dir(&dir)?;
        Ok(dir)
    }
}
