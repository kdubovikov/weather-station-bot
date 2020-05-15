#![feature(async_closure)]
#[macro_use]
extern crate diesel;

mod db;
mod schema;
mod settings;

use crate::settings::Settings;
use clap::{App, Arg};
use rumqtt::{MqttClient, MqttOptions, QoS, SecurityOptions, Notification, Receiver};
use std::fs::File;
use std::io::prelude::*;

use tokio::sync::{mpsc::UnboundedSender, watch};
use std::sync::{Arc, Mutex};

use tbot::{
    prelude::*,
    types::parameters::{ChatId, Text},
};

use db::{establish_connection, NewWeatherMessage, EspWeatherMessage, subscribe, unsubscribe, get_all_subscribers};


/// Helper function to read certificate files from disk
fn read_file_to_bytes(path: &str) -> Vec<u8> {
    let mut f = File::open(path).unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    buf
}

/// Connect to MQTT server using [Settings](settings::Settings) structure. The settings are meant to be read from config TOML file
/// Will automatically subsribe to the topic name in the config. 
async fn connect_to_mqtt_server(settings: &Settings) -> Receiver<Notification> {
    println!(
        "Conntcting to MQTT server at {}:{}/{}",
        settings.mqtt.host, settings.mqtt.port, settings.mqtt.topic_name
    );

    let ca_cert = read_file_to_bytes(&settings.tls.ca_cert);

    let mqtt_options = MqttOptions::new(
        "weather_station_bot",
        &settings.mqtt.host,
        settings.mqtt.port
    )
    .set_ca(ca_cert)
    .set_security_opts(SecurityOptions::UsernamePassword(
        settings.mqtt.username.clone(),
        settings.mqtt.password.clone(),
    ));

    let (mut mqtt_client, notifications) = MqttClient::start(mqtt_options).unwrap();

    mqtt_client
        .subscribe(&settings.mqtt.topic_name, QoS::AtLeastOnce)
        .unwrap();

    notifications

}

/// Main MQTT message processing loop. 
///
/// Recieves a message from MQTT topic, deserializes it and sends it for further processing using Tokio MPSC framwrok. See [send_message_to_telegram](send_message_to_telegram)
fn process_messages_from_device(notifications: &Receiver<Notification>, tok_tx: &UnboundedSender<EspWeatherMessage>) {
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
}

/// Sends a message to subscribers
async fn send_message_to_telegram(chat_id:i64, msg: &EspWeatherMessage, bot: &Arc<tbot::Bot>) {
    let message_str = &format!("{}", msg);
    let message = Text::plain(message_str);
    println!("Sending message to Telegram");
    bot.send_message(ChatId::from(chat_id), message)
        .call()
        .await
        .expect("Error while sending message to the bot");
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

    let bot = Arc::new(tbot::Bot::new(settings.telegram.token.clone()));

    let (tok_tx, mut tok_rx) = tokio::sync::mpsc::unbounded_channel::<EspWeatherMessage>();

    let (_, mut conf_rx) = watch::channel(settings.clone());

    let mut conf = conf_rx.clone();
    tokio::spawn(async move {
        let settings = conf.recv().await.unwrap();
        let notifications = connect_to_mqtt_server(&settings).await;

        tokio::task::spawn_blocking(move || {
            process_messages_from_device(&notifications, &tok_tx);
        });
    });


    println!("Waiting for messages");   
    let bot_sender = bot.clone();
    let mut conf = conf_rx.clone();
    tokio::spawn(async move {
        // let db = Arc::clone(&db);
        let settings: Settings = conf.recv().await.unwrap();
        while let Some(msg) = tok_rx.recv().await {
            let subscribers = get_all_subscribers(&establish_connection(&settings.db_path)); 
            println!("Recieved new message — {:?}", msg);
            let db_path = settings.db_path.clone();

            for subscriber in &subscribers {
                send_message_to_telegram(*subscriber, &msg, &bot_sender).await;
            }

            tokio::task::spawn_blocking(move || {
                println!("Saving message to DB");
                let connection = establish_connection(&db_path); 
                let new_log = NewWeatherMessage::from_esp_weather_message(&msg);
                new_log.save_to_db(&connection).unwrap();
                print!("Successfully saved message to DB");
            });
        }
    });

    let mut event_loop = (*bot).clone().event_loop();
    let mut conf = conf_rx.clone();

    event_loop.command("subscribe", move |context| {
        let mut conf = conf.clone();

        async move {
            let settings: Settings = conf.recv().await.unwrap();
            let chat_id = context.chat.id.0;
            context
                .send_message(&format!("Your chat id is {}", chat_id))
                .call()
                .await
                .err();
            

            let connection = establish_connection(&settings.db_path); 
            subscribe(chat_id, &connection).unwrap();
        }
    });

    let mut conf = conf_rx.clone();
    event_loop.command("unsubscribe", move |context| {
        let mut conf = conf_rx.clone();
        async move {
            let settings: Settings = conf.recv().await.unwrap();
            let chat_id = context.chat.id.0;
            let connection = establish_connection(&settings.db_path);
            let result = unsubscribe(chat_id, &connection);

            if result.is_ok() {
                context.send_message("Sucessfully unsubscribed").call().await.err();
            } else {
                context.send_message("Can't unsubscribe. Are you subscribed?").call().await.err();
            }
        }
    });

    event_loop.polling().start().await.unwrap();
}
