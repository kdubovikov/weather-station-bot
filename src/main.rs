#![feature(async_closure)]
mod settings;

use rumqtt::{MqttClient, MqttOptions, QoS, SecurityOptions};
use std::{sync::Arc};
use serde::{Serialize, Deserialize};
use std::fmt::Display;
use clap::{App, Arg};
use crate::settings::Settings;
use tbot::prelude::*;
use tbot::contexts::fields::Message;
use tokio::sync::mpsc;
use std::collections::HashMap;
use tbot::{errors::MethodCall, types::{chat, parameters::Text, parameters::ChatId, parameters::ImplicitChatId}};
use std::ops::Deref;
use tbot::types::Chat;

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

#[tokio::main]
async fn main() {
    let matches = App::new("Weather station bot")
                    .version("0.1.0")
                    .author("Kirill Dubovikov <dubovikov.kirill@gmail.com>")
                    .about("Telegram bot for ESP32 weather station")
                    .arg(Arg::with_name("config")
                        .short("c")
                        .long("config")
                        .value_name("FILE")
                        .help("Sets a custom config file")
                        .required(true)
                    ).get_matches();
    
    let config = matches.value_of("config").unwrap_or("config");
    let settings = Settings::new(config).expect("Error while reading settings"); 

    let mut bot = tbot::Bot::new(settings.telegram.token.clone());

    // let (mut tx, mut rx) = std::sync::mpsc::channel();
    let (mut tok_tx, mut tok_rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
            println!("Conntcting to MQTT server at {}:{}/{}", settings.mqtt.host, settings.mqtt.port, settings.mqtt.topic_name);
            let mut mqtt_options = MqttOptions::new("weather_station_bot", settings.mqtt.host, settings.mqtt.port);
            let (mut mqtt_client, notifications) = MqttClient::start(mqtt_options).unwrap();

            mqtt_client.subscribe(settings.mqtt.topic_name, QoS::AtLeastOnce).unwrap();

            tokio::task::spawn_blocking(move || {
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
                            // tx.send(msg);
                            tok_tx.send(msg);
                        }
                        _ => println!("{:?}", notification)
                    }
                }
            });
    });

    println!("Waiting for messages");
    while let Some(msg) = tok_rx.recv().await {
        println!("Recieved new message — {:?}", msg);
        let message_str = &format!("{}", msg);
        let message = Text::plain(message_str);
        bot.send_message(ChatId::from(114238258), message).call().await.expect("Error while sending message to the bot");
    }

    bot.event_loop().polling().start().await.unwrap();
}
