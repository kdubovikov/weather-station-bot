use serde::{Serialize, Deserialize};
use config::{self, ConfigError, };

#[derive(Serialize, Deserialize)]
pub struct MQTTSettings {
    pub host: String,
    pub port: u16,
    pub topic_name: String
}

#[derive(Serialize, Deserialize)]
pub struct TelegramSettings {
   pub token: String
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub mqtt: MQTTSettings,
    pub telegram: TelegramSettings
}

impl Settings {
   pub fn new(config_path: &str) -> Result<Self, ConfigError> {
    let mut settings = config::Config::default();
    println!("Reading config file");
    settings.merge(config::File::with_name(config_path)).unwrap();
    settings.try_into()
   }
}
