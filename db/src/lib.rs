#[macro_use]
extern crate diesel;
mod schema;

use crate::schema::weather_log;
use crate::schema::subscribers;
use crate::schema::weather_log::dsl::*;
use crate::schema::subscribers::dsl::*;
use chrono::Utc;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Connect to SQLite database
pub fn establish_connection(database_url: &str) -> SqliteConnection {
    println!("Connecting to {}", database_url);
    SqliteConnection::establish(database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

/// This struct represents an existing bot subscriber who can recieve 
/// notifications about new weather measurements.
/// `Queryable` trait identifies that Diesel will use this structure to 
/// represent rows selected from  SQLite database.
#[derive(Queryable)]
pub struct Subscriber {
    id: i32,
    telegram_chat_id: i64
}

/// This struct represents a new bot subscriber.
/// `Insertable` trait means that Diesel will expect this stcuture to
/// represent new rows that we can insert into SQLite database.
#[derive(Insertable)]
#[table_name = "subscribers"]
pub struct NewSubscriber {
    telegram_chat_id: i64
}

/// Save new subscriber to the database if he does not already exist
pub fn subscribe(chat_id: i64, connection: &SqliteConnection) -> Result<NewSubscriber, &str> {
    // let's find if there are any existing subscribers
    let existing_subscriber = subscribers.filter(telegram_chat_id.eq(chat_id)).first::<Subscriber>(connection);

    if let Err(diesel::NotFound) = existing_subscriber {
        // let's create a new one if there is noone found
        let subscriber = NewSubscriber {telegram_chat_id: chat_id };

        match diesel::insert_into(subscribers).values(&subscriber).execute(connection) {
            Ok(_) => Ok(subscriber),
            Err(_) => Err("Error while saving new subscriber to DB")
        }
    } else {
        Err("The subscriber already exists")
    }
}

/// Deletes a subscriber from the database
pub fn unsubscribe(chat_id: i64, connection: &SqliteConnection) -> QueryResult<usize> {
    diesel::delete(subscribers.filter(telegram_chat_id.eq(chat_id))).execute(connection)
}

/// returns all existing subscribers
pub fn get_all_subscribers(connection: &SqliteConnection) -> Vec<i64> {
    subscribers.load::<Subscriber>(connection)
        .unwrap_or_default()
        .iter()
        .map(|s| s.telegram_chat_id)
        .collect()
}


/// WeatherMessage representation for read DB queries
/// This structure represents weather data that we read from SQLlite
#[derive(Queryable, Serialize, Deserialize, Debug)]
pub struct WeatherMessage {
    pub id: i32,
    pub timestamp: String,
    pub temp: f32,
    pub pressure: f32,
    pub humidity: f32,
}

/// WeatherMessage respresentation for insert DB queries
/// This structure represents weather data that we insert into SQLlite
#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name = "weather_log"]
pub struct NewWeatherMessage {
    timestamp: String,
    temp: f32,
    pressure: f32,
    humidity: f32,
}

/// Raw WeatherMessage that comves from the edge device via MQTT in a JSON format
#[derive(Serialize, Deserialize, Debug)]
pub struct EspWeatherMessage {
    temp: f32,
    pressure: f32,
    humidity: f32,
}

impl EspWeatherMessage {
    /// We will use this function to convert temperature in Celcium degrees (because metric system rules 😅)
    pub fn temp_to_emoji(&self) -> &str {
        if self.temp < -10. {
            "🥶"
        } else if self.temp < 0. {
            "❄️"
        } else if self.temp > 20. {
            "☀️"
        } else if self.temp > 30. {
            "🔥"
        } else {
            ""
        }
    }

    /// A handy function to convert humidity percentage into useful notifications
    pub fn humidity_to_emoji(&self) -> &str {
        if self.humidity > 70. {
            "Do not forget an umbrella ☂️"
        } else if self.humidity > 90. {
            "🌧"
        } else {
            ""
        }
    }

    /// Converts pressure from Pascals to millimetres of mercury
    pub fn pressure_to_enoji(&self) -> &str {
        const PA_TO_MM_MERCURY: f32 = 133.322;
        const NORMAL_PRESSURE: f32 = 101_325.0 / PA_TO_MM_MERCURY;

        if (self.pressure / PA_TO_MM_MERCURY) > (NORMAL_PRESSURE + 10.0) {
            "⬆️ high pressure"
        } else if (self.pressure / PA_TO_MM_MERCURY) < (NORMAL_PRESSURE - 10.0) {
            "⬇️ low pressure"
        } else {
            ""
        }
    }
}

/// A few convenience methods for constructing and saving weather messages
impl NewWeatherMessage {
    /// Create a new WeatherMessage that later can be saved to database
    pub fn new(tmp: f32, press: f32, hum: f32) -> NewWeatherMessage {
        NewWeatherMessage {
            timestamp: Utc::now().to_rfc3339(),
            temp: tmp,
            pressure: press,
            humidity: hum,
        }
    }

    /// Converts `EspWeatherMessage` to `NewWeatherMessage` that can be saved to DB.
    /// This function should be used to convert all new incoming weather messages
    pub fn from_esp_weather_message(msg: &EspWeatherMessage) -> NewWeatherMessage {
        NewWeatherMessage::new(msg.temp, msg.humidity, msg.humidity)
    }
   
    /// Saves message to database
    pub fn save_to_db(&self, connection: &SqliteConnection) -> QueryResult<usize> {
        let result = diesel::insert_into(weather_log::table)
            .values(self)
            .execute(connection);
        
        result
    }
}

/// Display trait is used to convert `EspWeatherMessage` to string.
/// We will use this implementation to convert messages to text notifications that will be sent to subscribers
impl Display for EspWeatherMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const pa_to_mm_mercury: f32 = 133.322;

        write!(
            f,
            "{}{}{}\n℃{:>10.2}\nВлажность{:>10.2}%\nДавление{:>10.2} мм рт.с.",
            self.temp_to_emoji(),
            self.humidity_to_emoji(),
            self.pressure_to_enoji(),
            self.temp,
            self.humidity,
            self.pressure / pa_to_mm_mercury
        )
    }
}

/// Returns all existing weather messages ordered from last to first
pub fn get_all_weather_messages(connection: &SqliteConnection) -> Vec<WeatherMessage> {
    let weather_logs = weather_log.order(timestamp.desc()).load::<WeatherMessage>(connection);
    weather_logs.unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DB: &str = "test.sqlite";

    #[test]
    fn test_select_weather_message() {
        let connection = establish_connection(TEST_DB);
        let results = weather_log.load::<WeatherMessage>(&connection);
        assert!(results.is_ok())
    }

    #[test]
    fn test_insert_and_delete_weather_message() {
        let connection = establish_connection(TEST_DB);
        let new_weather_message = NewWeatherMessage::new(25.0, 10000.0, 55.0);
        let result = diesel::insert_into(weather_log::table)
            .values(&new_weather_message)
            .execute(&connection);

        assert!(result.is_ok());

        let last_log = weather_log
            .order(timestamp.desc())
            .load::<WeatherMessage>(&connection);
        assert!(last_log.is_ok());

        let last_log_id: i32 = last_log.unwrap()[0].id;
        let del_result = diesel::delete(weather_log.find(last_log_id)).execute(&connection);
        assert!(del_result.is_ok());

        let results = weather_log.load::<WeatherMessage>(&connection);
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[test]
    fn test_subscription() {
        const TEST_SUB_ID: i64 = 123;
        let connection = establish_connection(TEST_DB);
        let sub_result = subscribe(TEST_SUB_ID, &connection);
        if let Err(e) = sub_result {
            println!("{}", e);
        }
        assert!(sub_result.is_ok());

        let subscriber: Subscriber = subscribers.filter(telegram_chat_id.eq(TEST_SUB_ID)).first(&connection).unwrap();
        assert_eq!(subscriber.telegram_chat_id, TEST_SUB_ID);

        let unsub_result = unsubscribe(TEST_SUB_ID, &connection);
        assert!(unsub_result.is_ok());
        let result: QueryResult<Subscriber> = subscribers.filter(telegram_chat_id.eq(TEST_SUB_ID)).first(&connection);
        assert!(result.is_err());
    }
}