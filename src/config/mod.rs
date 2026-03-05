pub mod model;

pub use model::Config;

use std::path::Path;

use crate::error::NinetyNineError;

const CONFIG_FILE_NAME: &str = ".ninety-nine.toml";

pub fn load_config(project_root: &Path) -> Result<Config, NinetyNineError> {
    let config_path = project_root.join(CONFIG_FILE_NAME);

    if !config_path.exists() {
        return Ok(Config::default());
    }

    let contents = std::fs::read_to_string(&config_path).map_err(|source| {
        NinetyNineError::ConfigIo {
            path: config_path.clone(), // clone: needed after move into closure
            source,
        }
    })?;

    let config: Config =
        toml::from_str(&contents).map_err(|source| NinetyNineError::ConfigParse { source })?;

    Ok(config)
}

pub fn default_config_toml() -> Result<String, NinetyNineError> {
    let config = Config::default();
    toml::to_string_pretty(&config).map_err(|e| NinetyNineError::InvalidConfig {
        message: e.to_string(),
    })
}
