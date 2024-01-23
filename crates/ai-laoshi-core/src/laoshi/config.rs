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
    // NOTE: This file_bundles Vec<FileBundle> corresponds to our laoshi.toml properties:
    // [[file_bundles]]
    // bundle_name = "knowledge"
    // src_dir = "files"         # Relative to this .toml file location (i.e. -> laoshi/files)
    // src_globs = ["*.md"]
    // dst_ext = "md"
}

#[derive(Debug, Deserialize)]
pub(super) struct FileBundle {
    pub bundle_name: String,
    pub src_dir: String,
    pub src_globs: Vec<String>,
    pub dst_ext: String,
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
