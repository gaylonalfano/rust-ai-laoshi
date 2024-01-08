use async_openai::{
    config::OpenAIConfig,
    types::{AssistantObject, AssistantToolsRetrieval, CreateAssistantRequest},
    Assistants, Client,
};
use derive_more::{Deref, Display, From};

use crate::{Error, Result};

// region:       -- Constants

// Tell OpenAI to limit the query and serialize accordingly.
const DEFAULT_QUERY: &[(&str, &str)] = &[("limit", "100")];

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

/// Create a blank AssistantId
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
        create(oac, config).await?;
        Ok(assistant_id)
    } else {
        // We don't have an Assistant
        let assistant_name = config.name.clone();
        let assistant_id = create(oac, config).await?;
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
