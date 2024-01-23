// NOTE:!! This is a utility/service module that gets it state from the OAI Client,
// so no need to add extra state (trying to be stateless as possible). It's dumb.
// Later we'll create a "Buddy" state that's a little smarter and uses these
// Assistant services.
// NOTE: The codebase has been updated a lot, so I'm following
// the latest commits here:
// REF: https://github.com/rust10x/rust-ai-buddy/blob/main/crates/ai-buddy/src/ais/asst.rs
// NOTE: Per OpenAI Docs: https://platform.openai.com/docs/api-reference/threads
// Assistants = Build assistants that can call models and use tools to perform tasks.
// You run threads on given/assigned assistants.
// Threads = Create threads that assistants can interact with.
// Messages = Messages are passed within Threads

use crate::{
    ais::message::{self, get_text_content},
    ais::types::{AssistantId, FileId, ThreadId},
    Error, Result,
};
use async_openai::{
    config::OpenAIConfig,
    types::{
        AssistantObject, AssistantToolsRetrieval, CreateAssistantFileRequest,
        CreateAssistantRequest, CreateFileRequest, CreateRunRequest,
        CreateThreadRequest, ModifyAssistantRequest, OpenAIFile, RunStatus,
        ThreadObject,
    },
    Assistants, Client,
};
use console::Term;
use derive_more::{Deref, Display, From};
use serde::{Deserialize, Serialize};
use simple_fs::SPath;
use std::{
    collections::{HashMap, HashSet},
    thread::sleep,
    time::Duration,
};

// region:       -- Constants

// Tell OpenAI to limit the query and serialize accordingly.
const DEFAULT_QUERY: &[(&str, &str)] = &[("limit", "100")];
const POLLING_DURATION_MS: u64 = 500;

// endregion:    -- Constants

// region:       -- Types

// NOTE: This is config controls our Open AI model!
// By design, this is separate from our higher-level 'Laoshi'
// module configuration abstraction (see laoshi/config.rs).
pub struct CreateConfig {
    pub name: String,
    pub model: String,
}

// endregion:    -- Types

// region:       -- Assistant CRUD

/// Create a blank AssistantId
pub async fn create(
    oac: &Client<OpenAIConfig>,
    config: CreateConfig,
) -> Result<AssistantId> {
    let oa_assistants_obj: Assistants<'_, OpenAIConfig> = oac.assistants();

    let assistant_obj = oa_assistants_obj
        .create(CreateAssistantRequest {
            model: config.model,
            name: Some(config.name),
            tools: Some(vec![AssistantToolsRetrieval::default().into()]),
            ..Default::default()
        })
        .await?;

    Ok(assistant_obj.id.into())
}

/// Create or load existing AssistantId
// Q: Even with load_or_create_assistant(), we create multiple
// Assistants in the OAI Platform. Something triggers a new create(),
// even though my logs show it's LOADED an existing assistant...
// A: I had two calls to create() inside my if/else block!
pub async fn load_or_create_assistant(
    oac: &Client<OpenAIConfig>,
    config: CreateConfig,
    recreate: bool,
) -> Result<AssistantId> {
    let assistant_obj = first_by_name(oac, &config.name).await?;
    let mut assistant_id = assistant_obj.map(|o| AssistantId::from(o.id));

    // -- Delete assistant if recreate true & have assistant_id
    if let (true, Some(assistant_id_ref)) = (recreate, assistant_id.as_ref()) {
        delete(oac, assistant_id_ref).await?;
        // Set assistant_id to None using Option<T>.take()
        assistant_id.take();
        println!("Assistant {} deleted", config.name);
    }

    // -- Load or create assistant if needed
    // Could also use the let assistant_id = if let Some(assistant_id) = assistant_id {..} pattern
    if let Some(assistant_id) = assistant_id {
        // We already have the Assistant
        println!("Assistant {} loaded.", config.name);
        Ok(assistant_id)
    } else {
        // We don't have an Assistant so need to create
        // Q: Why create assistant_name var?
        // A: To print/log!
        let assistant_name = config.name.clone();
        let assistant_id = create(oac, config).await?;
        println!("Assistant {} created.", assistant_name);
        Ok(assistant_id)
    }
}

pub async fn first_by_name(
    oac: &Client<OpenAIConfig>,
    name: &str,
) -> Result<Option<AssistantObject>> {
    let oa_assistants_obj = oac.assistants();

    // NOTE: We'd change the query here for pagination
    let assistants = oa_assistants_obj.list(DEFAULT_QUERY).await?.data;

    let assistant_obj = assistants
        // NOTE: How this code works:
        // .into_iter() - Consume the Iterator
        // .as_ref() - We don't want ownership
        // .map() - To access inner AssistantObject so we can compare name values
        // .unwrap_or(false) - Unwrap bc find() expects a bool, not an Option
        // and pass false if the Option is None (name not found)
        .into_iter() // consume the Iter
        .find(|a| a.name.as_ref().map(|n| n == name).unwrap_or(false));

    Ok(assistant_obj)
}

/// The instructions we give to the Assistant
pub async fn upload_instructions(
    oac: &Client<OpenAIConfig>,
    assistant_id: &AssistantId,
    ix_content: String,
) -> Result<()> {
    let oa_assistants_obj = oac.assistants();
    let modify_request = ModifyAssistantRequest {
        instructions: Some(ix_content),
        ..Default::default()
    };

    // NOTE: AssistantId(String) implements Deref, so we can
    // pass assistant_id (&AssistantId) and it will get deref-ed
    // into a &str.
    oa_assistants_obj
        .update(assistant_id, modify_request)
        .await?;

    Ok(())
}

pub async fn delete(
    oac: &Client<OpenAIConfig>,
    assistant_id: &AssistantId,
) -> Result<()> {
    let oa_assistants_obj = oac.assistants();
    let oa_org_files_obj = oac.files();

    // -- Delete ORG files since our Assistant may have files associated with it
    // NOTE: TIP! There's a handy HashMap.into_values()
    for file_id in get_files_hashmap(&oac, assistant_id).await?.into_values() {
        // NOTE: !! The file might already be deleted, so we don't
        // have it stop/end with Err() by using '?' operator.
        let del_res = oa_org_files_obj.delete(&file_id).await;
        // TODO: Could consider 'match' instead & implement EventBus (AisEvent::OrgFileDeleted())
        // REF: https://github.com/rust10x/rust-ai-buddy/blob/main/crates/ai-buddy/src/ais/asst.rs
        if del_res.is_ok() {
            println!("File deleted - {file_id}");
        }
    }

    // NOTE: !! No need to delete associated/attached Assistant files since
    // we delete the full Assistant object from OpenAI.

    // -- Delete the assistant
    oa_assistants_obj.delete(assistant_id).await?;

    Ok(())
}

// endregion:    -- Assistant CRUD

// region:       -- Threads that Assistants can interact with
pub async fn create_thread(oac: &Client<OpenAIConfig>) -> Result<ThreadId> {
    let oa_threads_obj = oac.threads();
    let thread_obj = oa_threads_obj
        .create(CreateThreadRequest {
            ..Default::default()
        })
        .await?;

    Ok(thread_obj.id.into())
}

pub async fn get_thread(
    oac: &Client<OpenAIConfig>,
    thread_id: &ThreadId,
) -> Result<ThreadObject> {
    let oa_threads_obj = oac.threads();
    let thread_obj = oa_threads_obj.retrieve(thread_id).await?;

    Ok(thread_obj)
}

/// Send message to Thread/Conversation
// NOTE: Could extend this to also upload files.
// NOTE: We're keeping the messaging simple, but our
// Assistants can be more low-level that track conversation
// history/context, etc.
pub async fn run_thread_msg(
    oac: &Client<OpenAIConfig>,
    assistant_id: &AssistantId,
    thread_id: &ThreadId,
    msg: &str,
) -> Result<String> {
    // -- Create OpenAI Message
    let message_request = message::create_user_message(msg);

    // -- Attach message to thread
    let message_obj = oac
        .threads()
        .messages(thread_id)
        .create(message_request)
        .await?;

    // -- Create a run for the thread
    // NOTE: This is where you can configure model, ixs, tools, metadata
    let run_request = CreateRunRequest {
        assistant_id: assistant_id.to_string(),
        ..Default::default()
    };
    // NOTE: This sends the request to the API
    let run_obj = oac.threads().runs(thread_id).create(run_request).await?;

    // -- Loop through RunObject until you get a result and print execution
    // NOTE: This is where the 'console' crate comes in
    // Add some print/log statuses. However, the loop will flush it
    let term = Term::stdout();
    loop {
        // NOTE: Need to add new custom Error::IO enum variant for std errors
        term.write_str(">")?;
        let run_obj = oac.threads().runs(thread_id).retrieve(&run_obj.id).await?;
        term.write_str("< ")?;

        match run_obj.status {
            // NOTE: This returns only out of the match (not the whole function!)
            RunStatus::Queued | RunStatus::InProgress => (), // Continue looping
            RunStatus::Completed => {
                term.write_str("\n")?;
                // NOTE: This 'return' returns out of the whole function (not just the match!)
                return get_first_thread_message_content(oac, thread_id).await;
            }
            other => {
                term.write_str("\n")?;
                return Err(Error::RunError(other));
                // return Err(format!("ERROR WHILE RUN: {:?}", other).into());
            }
        }

        tokio::time::sleep(Duration::from_millis(POLLING_DURATION_MS)).await;
    }
}

// NOTE: Once we get an Ok() from the run_thread_msg(), we want
// the latest message of the thread.
pub async fn get_first_thread_message_content(
    oac: &Client<OpenAIConfig>,
    thread_id: &ThreadId,
) -> Result<String> {
    // -- Query the Thread for the latest (not the hundreds of older messages)
    // NOTE: We use the Messages::ListMessages
    // REF: https://platform.openai.com/docs/api-reference/messages/listMessages
    // REF: https://docs.rs/async-openai/0.18.0/async_openai/struct.Messages.html#method.list
    static QUERY: [(&str, &str); 1] = [("limit", "1")];

    let messages = oac.threads().messages(thread_id).list(&QUERY).await?;
    let message_obj = messages
        .data
        .into_iter()
        .next()
        .ok_or_else(|| Error::NoMessageFoundInMessages)?;

    let text = get_text_content(message_obj)?;

    Ok(text)
}

// endregion:    -- Threads that Assistants can interact with

// region:       -- Files
// WARN: The OpenAI Assistants:List Assistant Files Response Obj
// doesn't return the 'filename' property, so we need to do a bit
// of extra work to first hit the 'Files' endpoint and then we
// can use Assistants:List Assistant Files
// REF: https://platform.openai.com/docs/api-reference/files/list
// REF: https://platform.openai.com/docs/api-reference/assistants/listAssistantFiles
// REF: https://youtu.be/PHbCmIckV20?t=8249

/// Returns the file id by file name hashmap.
pub async fn get_files_hashmap(
    oac: &Client<OpenAIConfig>,
    assistant_id: &AssistantId,
) -> Result<HashMap<String, FileId>> {
    // -- Get all assistant files (these don't have a .name property sadly)
    let oa_assistants_obj = oac.assistants();
    let oa_assistant_files_obj = oa_assistants_obj.files(assistant_id);
    let assistant_files = oa_assistant_files_obj.list(DEFAULT_QUERY).await?.data;
    // NOTE: We only want files that belong to the passed assistant_id,
    // so we're going to create a HashSet<String>
    // REF: "id": "file-abc123"
    let assistant_file_ids: HashSet<String> =
        assistant_files.into_iter().map(|f| f.id).collect();

    // -- Get all files for org (these have .filename property)
    let oa_files_obj = oac.files();
    let org_files = oa_files_obj.list(DEFAULT_QUERY).await?.data;

    // -- Build the k:v file_name:file_id HashMap
    // Q: Iterator over org_files and map/filter to insert
    // into our HashSet??
    // U: Nope. Build a separate HashMap
    // NOTE: .filter() takes a REFERENCE
    // .map() takes based on .into()->REF, .into_iter()->OWNED
    let file_id_by_name_hm: HashMap<String, FileId> = org_files
        .into_iter()
        .filter(|org_file| assistant_file_ids.contains(&org_file.id))
        // NOTE: Should only have Assistant files at this point
        // Q: How to now perform a HashMap.insert(org_file.filename)??
        // A: We return sth (tuple) that can be collected into our HashMap<String, FileId>!
        // org_file.id.into() -> FileId(String)
        // REF: https://youtu.be/PHbCmIckV20?t=8670
        .map(|org_file| (org_file.filename, org_file.id.into()))
        .collect();

    Ok(file_id_by_name_hm)
}

/// Uploads a file to an assistant (first to the org account, then attaches to asst)
/// - `force` is `false`, will not upload the file if already uploaded.
/// - `force` is `true`, it will delete existing file (account and asst), and upload.
///
/// Returns `(FileId, has_been_uploaded)`
// NOTE: Again, this Assistant module is more lower-level compared
// to the custom Laoshi/Agent module, so we need our Assistant
// to support files before we implement into Laoshi module.
// NOTE: Assistant know nothing about bundling files. It simply
// will upload a file to OpenAI for a given AssistantId.
pub async fn upload_file_by_name(
    oac: &Client<OpenAIConfig>,
    assistant_id: &AssistantId,
    file: &SPath,
    force: bool,
) -> Result<(FileId, bool)> {
    let file_name = file.file_name();

    // Q: Get the HashMap of Assistant files and then
    // look for a match on file_name?
    // U: Kinda... Need to use if let Some(file_id) or if let Err(err) more...
    let mut assistant_files_hm = get_files_hashmap(&oac, assistant_id).await?;
    // Q: Why remove() instead of just get()?
    // A: Because we need an owned Option<FileId> and don't need the HM afterwards.
    // If we use get(), it gives a ref Option<&FileId> and then we'll need
    // to figure out how to clone() it or something more complicated when
    // we eventually return Ok((file_id, t/f)).
    let file_id = assistant_files_hm.remove(file_name);

    // -- If force is `false` and file already exists (uploaded), return early
    if !force {
        if let Some(file_id) = file_id {
            return Ok((file_id, false));
        }
    }

    // -- If file already exists (old) and force is true, delete file & assistant file association
    if let Some(file_id) = file_id {
        // -- Delete the org file
        let oa_org_files_obj = oac.files();
        if let Err(err) = oa_org_files_obj.delete(&file_id).await {
            eprintln!("X Can't delete file '{}'\n    cause: {}", file_name, err);
        }

        // -- Delete the Assistant file association
        let oa_assistants_obj = oac.assistants();
        let oa_assistant_files_obj = oa_assistants_obj.files(assistant_id);
        if let Err(err) = oa_assistant_files_obj.delete(&file_id).await {
            eprintln!(
                "X Can't delete assistant file '{}'\n    cause: {}",
                file_name, err
            )
        }
    }

    // -- Upload file to OpenAI org account
    // Use the terminal to display output to user
    let term = Term::stdout();
    term.write_line(&format!("* Uploading file '{}'", file_name))?;

    // Upload file
    let oa_org_files_obj = oac.files();
    // Q: Use if let Err() or if let Some()?
    let oa_org_file_obj = oa_org_files_obj
        .create(CreateFileRequest {
            file: file.into(),
            purpose: "assistants".into(),
        })
        .await?;
    // Update terminal print
    term.clear_last_lines(1)?;
    term.write_line(&format!("* Uploaded file '{}'", file_name))?;

    // -- Attach file to specified Assistant
    let oa_assistants_obj = oac.assistants();
    let oa_assistant_files_obj = oa_assistants_obj.files(assistant_id);
    let assistant_file_obj = oa_assistant_files_obj
        .create(CreateAssistantFileRequest {
            file_id: oa_org_file_obj.id.clone(),
        })
        .await?;

    // -- Assert warning if org file doesn't match assistant file
    if oa_org_file_obj.id != assistant_file_obj.id {
        println!(
            "SHOULD NOT HAPPEN! File id not matching {} {}",
            oa_org_file_obj.id, assistant_file_obj.id
        )
    }

    Ok((assistant_file_obj.id.into(), true))
}

// endregion:    -- Files
