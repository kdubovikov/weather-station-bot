/// WeatherStation Telegram Bot
#![feature(async_closure)]

use tokio::stream::StreamExt;
use weather_station_bot::Settings;
use clap::{App, Arg};
use std::fs::File;
use std::io::prelude::*;

use tokio::sync::{mpsc::{UnboundedSender, channel}, watch};
use std::sync::Arc;

use tbot::{
    prelude::*,
    types::parameters::{ChatId, Text},
};

use db::{establish_connection, NewWeatherMessage, EspWeatherMessage, subscribe, unsubscribe, get_all_subscribers};
use rumq_client::{self, eventloop, MqttOptions, QoS, Request, Subscribe, Notification};


/// Helper function to read certificate files from disk
fn read_file_to_bytes(path: &str) -> Vec<u8> {
    let mut f = File::open(path).unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    buf
}

/// Connects to MQTT server using [Settings](settings::Settings) structure. The settings are meant to be read from config TOML file
/// Will automatically subsribe to the topic name in the config.
/// Subscribes to the weather topic and forwards parsed messages to tokio channel
async fn process_mqtt_messages(settings: &Settings, tx: UnboundedSender<EspWeatherMessage>) {
    println!(
        "Conntcting to MQTT server at {}:{}/{}",
        settings.mqtt.host, settings.mqtt.port, settings.mqtt.topic_name
    );

    // Create MQTT connection options using information from config file
    let mut mqtt_options = MqttOptions::new("weather_station_bot", settings.mqtt.host.clone(), settings.mqtt.port.clone());
    mqtt_options.set_credentials(settings.mqtt.username.clone(), settings.mqtt.password.clone());
    mqtt_options.set_inflight(10);

    let ca_cert = read_file_to_bytes(&settings.tls.ca_cert);
    mqtt_options.set_ca(ca_cert);
    mqtt_options.set_keep_alive(50);
    mqtt_options.set_throttle(std::time::Duration::from_secs(1));

    // requests_tx will be used to send subscription requests to the MQTT server
    // requests_rx will be used by tokio event loop to recieve new messages
    let (mut requests_tx, requests_rx) = channel(10);

    // Here we subscribe to the MQTT topic from the config file
    let subscription = Subscribe::new(settings.mqtt.topic_name.clone(), QoS::AtLeastOnce);
    let _ = requests_tx.send(Request::Subscribe(subscription)).await;

    // And create the Tokio event loop which drives the whole message processing
    let mut event_loop = eventloop(mqtt_options, requests_rx);
    let mut stream = event_loop.connect().await.unwrap();

    // At last, we delegate each new message process_message_from_device function
    println!("Waiting for notifications");
    while let Some(notification) = stream.next().await {
        println!("New notification — {:?}", notification);
        process_message_from_device(&notification, &tx);
    }
}

/// Main MQTT message processing loop. 
///
/// Recieves a message from MQTT topic, deserializes it and sends it for further processing using Tokio MPSC framwrok. See [send_message_to_telegram](send_message_to_telegram)
fn process_message_from_device(notification: &Notification, tok_tx: &UnboundedSender<EspWeatherMessage>) {
    match notification {
        // Notification::Publish represents a message published in MQTT topic
        Notification::Publish(publish) => {
            let text: String = String::from_utf8(publish.payload.clone())
                .expect("Can't decode payload for notification");
            println!("Recieved message: {}", text);

            // As you remember, our ESP32 board encodes messages in JSON format and sends then to the MQTT server.
            // Here, we decode (deserialize) this message into Rust struct `EspWeatherMessage`
            let msg: EspWeatherMessage = serde_json::from_str(&text)
                .expect("Error while deserializing message from ESP");
            println!("Deserialized message: {:?}", msg);
            println!("{}", msg);

            // We send deserialized message via Tokio channel, that allows different coroutines to communicate between each other
            tok_tx.send(msg).unwrap();
        }
        _ => println!("{:?}", notification),
    }
}

/// Sends a message to subscribers
async fn send_message_to_telegram(chat_id:i64, msg: &EspWeatherMessage, bot: &Arc<tbot::Bot>) {
    // First, we convert EspWeatherMessage to string. Since we have implemented Diplay trait, we can just use format! macro
    let message_str = &format!("{}", msg);
    // Text::plain is used in tbot Telegram library to wrap plain text messages
    let message = Text::plain(message_str);
    println!("Sending message to Telegram");

    // Here, we send the message to a subscriber's chat
    bot.send_message(ChatId::from(chat_id), message)
        .call()
        .await // send_message is asynchronous, to actually call it and wait for it's result we need to use await
        .expect("Error while sending message to the bot");
}

// Main WeatherStation Telegram bot fucntion
#[tokio::main]
async fn main() {
    // Clap library allows us to declaratively construct Command-Line Interfaces.
    // Here we use it to display application medatada and allow user to specify config path
    let matches = App::new("Weather station bot")
        .version("0.1.0")
        .author("Kirill Dubovikov <dubovikov.kirill@gmail.com>")
        .about("Telegram bot for ESP32 weather station")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                // Notice that we can set FILE types for arguments, so that clap will validate user input for us.
                // If the user will enter invalid path clap will notify him and end the application
                .value_name("FILE") 
                .help("Sets a custom config file")
                .required(true),
        )
        .get_matches();

    println!("⚠️Do not forget to make sure that you can connect to Telegram APIs. The polling module won't time out if the service is unawailable");
    
    // Read settings from the config file. "config" crate makes this simple
    let config = matches.value_of("config").unwrap_or("config");
    let settings = Settings::new(config).expect("Error while reading settings");
    
    // Structure that represents our Telegram bot.
    // It is wrapped in an Arc (Atomic reference counter) because we will use it later in send_message_to_telegram function.
    // This function is asynchronous, so Tokio could run it in a different thread.
    // Rust compiler is very smart and it won't allow us to pass values between different threads
    // without proper tracking of references and synchronization, which Arc provices for us.
    let bot = Arc::new(tbot::Bot::new(settings.telegram.token.clone()));

    // Tokio unbounded_channel is used to communicate between different asynchronous functions which may run in different threads.
    // Channels are like pipes: tok_tx can be used to send messages down the piple, and tok_rx can be used to recieve them
    let (tok_tx, mut tok_rx) = tokio::sync::mpsc::unbounded_channel::<EspWeatherMessage>();

    // watch::channel is a Tokio channel with a single producer and multiple consumers.
    // This is useful to share configuration (single producer) with many asynchronous functions (multiple consumers)
    let (_, conf_rx) = watch::channel(settings.clone());

    let mut conf = conf_rx.clone();
    
    // That's how we can recieve a config from watch::channel
    let settings = conf.recv().await.unwrap();

    // tokio::spawn runs process_mqtt_messages asynchronously so that it won't block our main thread.
    // This means that the execution of this function can continue without interruption.
    //
    // async move designates that we can use variables from our main thread inside tokio::spawn block
    // If you are familiar with closures and Rust borrow checker: 
    // async move tells that we can move values from enclosing environment inside the clousure's environment
    tokio::spawn(async move {
        process_mqtt_messages(&settings, tok_tx).await;
    });

    println!("Waiting for messages");   
    // We clone some variables since we need to move them into closure, but we allso will need them later
    // Alternatively, you can use Arc's or channels to curcumvent cloning, but I have decided to
    // make things simpler since cloning values a constant number of times at the application start
    // won't be a bottleneck in our case
    let bot_sender = bot.clone();
    let mut conf = conf_rx.clone();
    tokio::spawn(async move {
        // Here all the magic happens 🌈
        let settings: Settings = conf.recv().await.unwrap();
        // Recieve new message from MQTT topic
        while let Some(msg) = tok_rx.recv().await {
            // Get all subrcribers from database
            let subscribers = get_all_subscribers(&establish_connection(&settings.db_path)); 
            println!("Recieved new message — {:?}", msg);
            let db_path = settings.db_path.clone();

            // Send message to all active subscribers
            for subscriber in &subscribers {
                send_message_to_telegram(*subscriber, &msg, &bot_sender).await;
            }

            // Save weather data to database. Here we use a spawn blocking function to execute blocking code
            // which won't normally work in an async block
            tokio::task::spawn_blocking(move || {
                println!("Saving message to DB");
                let connection = establish_connection(&db_path); 
                // Convert ESPWeatherMessage to NewWeatherMessage which can be used by diesel framework
                // to save weather data to database
                let new_log = NewWeatherMessage::from_esp_weather_message(&msg);
                new_log.save_to_db(&connection).unwrap();
                print!("Successfully saved message to DB");
            });
        }
    });

    // Here we get the event loop to register some commands for the bot users
    let mut event_loop = (*bot).clone().event_loop();
    let conf = conf_rx.clone();
    event_loop.command("subscribe", move |context| {
        // Subscribe command can be used by the user to get notifications about
        // new weather readings
        let mut conf = conf.clone();

        async move {
            let settings: Settings = conf.recv().await.unwrap();
            // Here we get the user's chat_id
            let chat_id = context.chat.id.0;
            context
                .send_message(&format!("Your chat id is {}", chat_id))
                .call()
                .await
                .err();
            

            // and save it to the database
            let connection = establish_connection(&settings.db_path); 
            subscribe(chat_id, &connection).unwrap();
        }
    });

    let conf = conf_rx.clone();
    event_loop.command("unsubscribe", move |context| {
        // the unsubscribe command can be used to unsubscribe from all bot notifications
        let mut conf = conf.clone();
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

    // this starts the main event loop
    event_loop.polling().start().await.unwrap();
}
