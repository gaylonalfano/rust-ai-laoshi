// NOTE: Messages are within Threads
use crate::{Error, Result};
use async_openai::types::{CreateMessageRequest, MessageContent, MessageObject};

// region:       -- Message Constructors

pub fn create_user_message(content: impl Into<String>) -> CreateMessageRequest {
    CreateMessageRequest {
        role: "user".to_string(),
        content: content.into(),
        ..Default::default()
    }
}

// endregion:    -- Message Constructors

// region:       -- Context Extractor

// NOTE: Q: We use this to get the latest thread msg content
// from inside the run_thread loop?
pub fn get_text_content(message_obj: MessageObject) -> Result<String> {
    // -- Get the first content item
    // NOTE: TIP! Best practice is return an Error if we can't retrieve.
    let msg_content = message_obj
        .content
        .into_iter()
        .next()
        .ok_or_else(|| Error::NoMessageInMessageObjectContent)?;

    // -- Get the text (fail if image)
    // NOTE: MessageContent::Text(MessageContentTextObject)
    // MessageContentTextObject.text.value
    // Q: How to simply extract the Text variant's inner data?
    // A: You literally return an Error if it's ImageFile variant!
    let msg_content_text = match msg_content {
        MessageContent::Text(inner) => inner.text.value,
        MessageContent::ImageFile(_) => return Err(Error::MessageImageNotSupported),
    };

    Ok(msg_content_text)
}

// endregion:    -- Context Extractor
