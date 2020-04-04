use rumqtt::{MqttClient, MqttOptions, QoS};
use std::{sync::Arc, convert::TryInto};

fn main() {
    let mut settings = config::Config::default();
    println!("Reading config file");
    settings.merge(config::File::with_name("config")).unwrap();

    let host = settings.get_str("mqtt_host").expect("Add mqtt_host to config.toml");
    let port: u16 = settings.get_int("mqtt_port").expect("Add mqtt_port to config.toml").try_into().unwrap();
    let topic_name = settings.get_str("topic_name").expect("Add topic_name to config.toml");

    println!("Conntcting to MQTT server at {}:{}/{}", host, port, topic_name);
    let mqtt_options = MqttOptions::new("weather_station_bot", host, port);
    let (mut mqtt_client, notifications) = MqttClient::start(mqtt_options).unwrap();
      
    mqtt_client.subscribe(topic_name, QoS::AtLeastOnce).unwrap();

    println!("Waiting for notifications");
    for notification in notifications {
        
        match notification {
            rumqtt::Notification::Publish(publish) => {
                let payload = Arc::try_unwrap(publish.payload).unwrap();
                let text: String = String::from_utf8(payload).expect("Can't decode payload for notification"); 
                println!("Recieved message: {}", text);
            }
            _ => println!("{:?}", notification)
        }
    }
}
