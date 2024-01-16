// NOTE: This is parsing our high-level laoshi.toml config with serde
use serde::Deserialize;

use crate::ais::assistant;

// Q: What's the difference btw pub(super) and pub(crate)?
#[derive(Debug, Deserialize)]
pub(super) struct Config {
    pub name: String,
    pub model: String,
    pub instructions_file: String,
    pub file_bundles: Vec<FileBundle>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FileBundle {
    bundle_name: String,
    src_dir: String,
    src_globs: Vec<String>,
    dst_ext: String,
}

// region:       -- Froms
// NOTE: By design, this is separate from our higher-level 'Laoshi'
// module configuration abstraction (see laoshi/config.rs), which itself
// is pulling from whatever we have in laoshi/laoshi.toml.
// So, we need to convert from this Config -> assistant::CreateConfig.
// REF: https://youtu.be/PHbCmIckV20?t=4253
impl From<&Config> for assistant::CreateConfig {
    fn from(config: &Config) -> Self {
        Self {
            name: config.name.clone(),
            model: config.model.clone(),
        }
    }
}

// endregion:    -- Froms
