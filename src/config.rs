use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;
use log::warn;

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub cache_dir: String,
    pub dns_enabled: bool,
    pub server_ip: Ipv4Addr,
    pub upstream_dns: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_dir: "./cache".to_string(),
            dns_enabled: true,
            server_ip: Ipv4Addr::new(127, 0, 0, 1),
            upstream_dns: "1.1.1.1".to_string(),
        }
    }
}

pub fn load_or_create_config() -> Config {
    let config_path = "config.toml";
    if !Path::new(config_path).exists() {
        let default_config = Config::default();
        let toml_string = toml::to_string_pretty(&default_config).unwrap();
        fs::write(config_path, toml_string).expect("Failed to create config.toml");
        warn!("Created default config.toml. Edit it and restart with 'sudo'.");
    }
    let config_str = fs::read_to_string(config_path).expect("Could not read config.toml");
    toml::from_str(&config_str).expect("Error parsing config.toml")
}
