use derive_more::{Deref, Display, From};
use serde::{Deserialize, Serialize};

// NOTE: TIP! -- Since we're going to have different objects (Assistant, Threads, etc.),
// the last thing we want is to have 'String' as the type. This always leads to bugs
// where pass a String ID of something into another String ID of something else.
// Therefore, we create a separate struct AssistantId(String).
// You could even consider using Arc<String> (you'd have to implement your own 'From'),
// but this would make it easy to have multi tasks in async and move ID across threads.
// REF: https://youtu.be/PHbCmIckV20?t=999
#[derive(Debug, From, Deref, Display)]
pub struct AssistantId(String);

#[derive(Debug, From, Deref, Serialize, Deserialize, Display)]
pub struct ThreadId(String);

#[derive(Debug, From, Deref, Display)]
pub struct FileId(String);
