#[macro_use]
extern crate tower_web;
extern crate tokio;

use clap::{App, Arg};
use tower_web::ServiceBuilder;
use tower_web::middleware::cors::{CorsBuilder, AllowedOrigins};
use tokio::prelude::*;
use weather_station_bot::Settings;

use db::{establish_connection, WeatherMessage, get_all_weather_messages};

/// This type will be part of the web service as a resource.
#[derive(Clone, Debug)]
struct WeatherApi;

/// This will be the JSON response
#[derive(Response)]
struct WeatherMessageResponse {
    messages: Vec<WeatherMessage>
}

impl_web! {
    impl WeatherApi {
        #[get("/")]
        #[content_type("json")]
        fn get_all_weather_messages(&self) -> Result<WeatherMessageResponse, ()> {
            let conn = establish_connection("./db.sqlite");
            let weather_messages = get_all_weather_messages(&conn);
            Ok(
               WeatherMessageResponse { messages: weather_messages }
            )
        }
    }
}

pub fn main() {
    let matches = App::new("Weather station bot")
        .version("1.0.0")
        .author("Kirill Dubovikov <dubovikov.kirill@gmail.com>")
        .about("REST API for ESP32 weather station")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .required(true),
        )
        .get_matches();
    
    let config = matches.value_of("config").unwrap_or("config");
    let settings = Settings::new(config).expect("Error while reading settings");

    let addr = format!("{}:{}", settings.api.host, settings.api.port).parse().expect("Invalid address");

    println!("Listening on http://{}", addr);

    let cors = CorsBuilder::new()
        .allow_origins(AllowedOrigins::Any { allow_null: true } )
        .build();

    ServiceBuilder::new()
        .resource(WeatherApi)
        .middleware(cors)
        .run(&addr)
        .unwrap();
}