// FIXME: This is a utility/service module that gets it state from the OAI Client,
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
    Error, Result,
};
use async_openai::{
    config::OpenAIConfig,
    types::{
        AssistantObject, AssistantToolsRetrieval, CreateAssistantRequest,
        CreateRunRequest, CreateThreadRequest, ModifyAssistantRequest, RunStatus,
        ThreadObject,
    },
    Assistants, Client,
};
use console::Term;
use derive_more::{Deref, Display, From};
use std::{thread::sleep, time::Duration};

// region:       -- Constants

// Tell OpenAI to limit the query and serialize accordingly.
const DEFAULT_QUERY: &[(&str, &str)] = &[("limit", "100")];
const POLLING_DURATION_MS: u64 = 500;

// endregion:    -- Constants

// region:       -- Types

// NOTE: This is config controls our Open AI model!
pub struct CreateConfig {
    pub name: String,
    pub model: String,
}

// NOTE: TIP! -- Since we're going to have different objects (Assistant, Threads, etc.),
// the last thing we want is to have 'String' as the type. This always leads to bugs
// where pass a String ID of something into another String ID of something else.
// Therefore, we create a separate struct AssistantId(String).
// You could even consider using Arc<String> (you'd have to implement your own 'From'),
// but this would make it easy to have multi tasks in async and move ID across threads.
// REF: https://youtu.be/PHbCmIckV20?t=999
#[derive(Debug, From, Deref, Display)]
pub struct AssistantId(String);

#[derive(Debug, From, Deref, Display)]
pub struct ThreadId(String);

#[derive(Debug, From, Deref, Display)]
pub struct FileId(String);

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

    // TODO: Delete files since our Assistant may have files associated with it

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
