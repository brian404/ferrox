use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_listen")]
    pub listen: String,

    #[serde(default = "default_proxy_upstream")]
    pub proxy_upstream: String,

    #[serde(default)]
    pub tls_cert: Option<String>,

    #[serde(default)]
    pub tls_key: Option<String>,
}

fn default_listen() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_proxy_upstream() -> String {
    "127.0.0.1:8000".to_string()
}

const DEFAULT_CONFIG_PATH: &str = "ferrox.conf";

fn resolve_config_path() -> (PathBuf, bool) {
    let args: Vec<String> = std::env::args().collect();

    for i in 0..args.len() {
        if args[i] == "--config" {
            if let Some(path) = args.get(i + 1) {
                return (PathBuf::from(path), true);
            }
        } else if let Some(path) = args[i].strip_prefix("--config=") {
            return (PathBuf::from(path), true);
        }
    }

    if let Ok(path) = std::env::var("FERROX_CONFIG") {
        return (PathBuf::from(path), true);
    }

    (PathBuf::from(DEFAULT_CONFIG_PATH), false)
}

pub fn load() -> Result<Config> {
    let (path, explicit) = resolve_config_path();

    if !path.exists() {
        if explicit {
            anyhow::bail!(
                "Config file not found: {} (given via --config or FERROX_CONFIG)",
                path.display()
            );
        }

        tracing::info!("No config found, creating default at {}", path.display());

        let default_conf = format!(
            r#"listen = "{}"
proxy_upstream = "{}"
"#,
            default_listen(),
            default_proxy_upstream()
        );

        let mut file = File::create(&path)?;
        file.write_all(default_conf.as_bytes())?;
        tracing::info!("Default config created at {}", path.display());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Invalid TOML in {}", path.display()))?;

    tracing::info!("Loaded config from {}", path.display());

    Ok(config)
}
