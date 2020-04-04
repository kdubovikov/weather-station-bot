use rumqtt::{MqttClient, MqttOptions, QoS};
use std::{sync::Arc, convert::TryInto, fmt::Display};
use serde::{Serialize, Deserialize};
use config::ConfigError;

#[derive(Serialize, Deserialize)]
struct MQTTSettings {
    host: String,
    port: u16,
    topic_name: String
}

#[derive(Serialize, Deserialize)]
struct TelegramSettings {
    token: String
}

#[derive(Serialize, Deserialize)]
struct Settings {
    mqtt: MQTTSettings,
    telegram: TelegramSettings
}

impl Settings {
   pub fn new() -> Result<Self, ConfigError> {
    let mut settings = config::Config::default();
    println!("Reading config file");
    settings.merge(config::File::with_name("config")).unwrap();
    settings.try_into()
   }
}

#[derive(Serialize, Deserialize, Debug)]
struct WeatherMessage {
    temp: f32,
    pressure: f32,
    altitude: f32,
    humidity: f32
}

impl WeatherMessage {
    fn temp_to_emoji(&self) -> &str {
        if self.temp < -10. {
            "🥶"
        } else if self.temp < 0. {
            "❄️"
        } else if self.temp > 20.  {
            "☀️"
        } else if self.temp > 30. {
            "🔥"
        } else {
            ""
        }
    }

    fn humidity_to_emoji(&self) -> &str {
        if self.humidity > 70. {
            "не забудь зонтик ☂️"
        } else if self.humidity > 90. {
            "🌧"
        } else {
            ""
        }
    }

    fn should_alert(&self) -> bool {
        self.temp > 30. || self.temp < 15. || self.humidity >= 85.
    }
}

impl Display for WeatherMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
        write!(f, "{}{}\n℃{:>10.2}\nВлажность{:>10.2}%\nДавление{:>10.2}", 
                  self.temp_to_emoji(), self.humidity_to_emoji(), self.temp, self.humidity, self.pressure)
     }

}

fn main() {
    let mut settings = Settings::new().expect("Error while reading settings"); 

    println!("Conntcting to MQTT server at {}:{}/{}", settings.mqtt.host, settings.mqtt.port, settings.mqtt.topic_name);
    let mqtt_options = MqttOptions::new("weather_station_bot", settings.mqtt.host, settings.mqtt.port);
    let (mut mqtt_client, notifications) = MqttClient::start(mqtt_options).unwrap();
      
    mqtt_client.subscribe(settings.mqtt.topic_name, QoS::AtLeastOnce).unwrap();

    println!("Waiting for notifications");
    for notification in notifications {
        
        match notification {
            rumqtt::Notification::Publish(publish) => {
                let payload = Arc::try_unwrap(publish.payload).unwrap();
                let text: String = String::from_utf8(payload).expect("Can't decode payload for notification"); 
                println!("Recieved message: {}", text);
                let msg: WeatherMessage = serde_json::from_str(&text).expect("Error while deserializing message from ESP");
                println!("Deserialized message: {:?}", msg);
                println!("{}", msg);
            }
            _ => println!("{:?}", notification)
        }
    }
}
