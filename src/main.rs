#![feature(async_closure)]
#[macro_use]
extern crate diesel;

mod db;
mod schema;
mod settings;

use crate::settings::Settings;
use clap::{App, Arg};
use rumqtt::{MqttClient, MqttOptions, QoS, SecurityOptions};
use std::fs::File;
use std::io::prelude::*;
use std::sync::Arc;

use tbot::{
    prelude::*,
    types::parameters::{ChatId, Text},
};

use db::{establish_connection, NewWeatherMessage, WeatherMessage, EspWeatherMessage};


fn read_file_to_bytes(path: &str) -> Vec<u8> {
    let mut f = File::open(path).unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    buf
}

#[tokio::main]
async fn main() {
    let matches = App::new("Weather station bot")
        .version("0.1.0")
        .author("Kirill Dubovikov <dubovikov.kirill@gmail.com>")
        .about("Telegram bot for ESP32 weather station")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .required(true),
        )
        .get_matches();

    println!("⚠️Do not forget to make sure that you can connect to Telegram APIs. The polling module won't time out if the service is unawailabel");

    let config = matches.value_of("config").unwrap_or("config");
    let settings = Settings::new(config).expect("Error while reading settings");

    let bot = tbot::Bot::new(settings.telegram.token.clone());

    let (tok_tx, mut tok_rx) = tokio::sync::mpsc::unbounded_channel::<EspWeatherMessage>();

    let db_path = settings.db_path.clone();

    tokio::spawn(async move {
        println!(
            "Conntcting to MQTT server at {}:{}/{}",
            settings.mqtt.host, settings.mqtt.port, settings.mqtt.topic_name
        );

        let ca_cert = read_file_to_bytes(&settings.tls.ca_cert);

        let mqtt_options = MqttOptions::new(
            "weather_station_bot",
            settings.mqtt.host,
            settings.mqtt.port,
        )
        .set_ca(ca_cert)
        // .set_client_auth(client_cert, client_key)
        .set_security_opts(SecurityOptions::UsernamePassword(
            settings.mqtt.username,
            settings.mqtt.password,
        ));

        let (mut mqtt_client, notifications) = MqttClient::start(mqtt_options).unwrap();

        mqtt_client
            .subscribe(settings.mqtt.topic_name, QoS::AtLeastOnce)
            .unwrap();

        tokio::task::spawn_blocking(move || {
            println!("Waiting for notifications");
            for notification in notifications {
                match notification {
                    rumqtt::Notification::Publish(publish) => {
                        let payload = Arc::try_unwrap(publish.payload).unwrap();
                        let text: String = String::from_utf8(payload)
                            .expect("Can't decode payload for notification");
                        println!("Recieved message: {}", text);
                        let msg: EspWeatherMessage = serde_json::from_str(&text)
                            .expect("Error while deserializing message from ESP");
                        println!("Deserialized message: {:?}", msg);
                        println!("{}", msg);
                        tok_tx.send(msg).unwrap();
                    }
                    _ => println!("{:?}", notification),
                }
            }
        });
    });

    println!("Waiting for messages");
    while let Some(msg) = tok_rx.recv().await {
        println!("Recieved new message — {:?}", msg);

        let message_str = &format!("{}", msg);
        let message = Text::plain(message_str);
        println!("Sending message to Telegram");
        bot.send_message(ChatId::from(114238258), message)
            .call()
            .await
            .expect("Error while sending message to the bot");

        let c = db_path.clone();

        tokio::task::spawn_blocking(move || {
            println!("Saving message to DB");
            let connection = establish_connection(&c); 
            let new_log = msg.to_new_weather_message();
            new_log.save_to_db(&connection).unwrap();
            print!("Successfully saved message to DB");
        });
    }

    let mut event_loop = bot.event_loop();

    event_loop.command("subscribe", |context| async move {
        let chat_id = context.chat.id;
        context
            .send_message(&format!("Your chat id is {}", chat_id))
            .call()
            .await
            .err();
    });

    event_loop.polling().start().await.unwrap();
}
