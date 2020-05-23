use serde::{Serialize, Deserialize};
use config::{self, ConfigError, };

#[derive(Serialize, Deserialize, Clone)]
pub struct MQTTSettings {
    pub host: String,
    pub port: u16,
    pub topic_name: String,
    pub username: String,
    pub password: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TelegramSettings {
   pub token: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TLSSettings {
    pub ca_cert: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub db_path: String,
    pub mqtt: MQTTSettings,
    pub telegram: TelegramSettings,
    pub tls: TLSSettings,
}

impl Settings {
   /// Read settings from the config file
   pub fn new(config_path: &str) -> Result<Self, ConfigError> {
    let mut settings = config::Config::default();
    println!("Reading config file");
    settings.merge(config::File::with_name(config_path)).unwrap();
    settings.try_into()
   }
}
