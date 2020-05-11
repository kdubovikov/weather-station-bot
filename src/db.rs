use crate::schema::weather_log;
use crate::schema::weather_log::dsl::*;
use chrono::Utc;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub fn establish_connection(database_url: &str) -> SqliteConnection {
    println!("Connecting to {}", database_url);
    SqliteConnection::establish(database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

#[derive(Queryable, Serialize, Deserialize, Debug)]
pub struct WeatherMessage {
    id: i32,
    timestamp: String,
    temp: f32,
    pressure: f32,
    humidity: f32,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name = "weather_log"]
pub struct NewWeatherMessage {
    timestamp: String,
    temp: f32,
    pressure: f32,
    humidity: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EspWeatherMessage {
    temp: f32,
    pressure: f32,
    humidity: f32,
}

impl EspWeatherMessage {
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

    pub fn humidity_to_emoji(&self) -> &str {
        if self.humidity > 70. {
            "не забудь зонтик ☂️"
        } else if self.humidity > 90. {
            "🌧"
        } else {
            ""
        }
    }

    pub fn pressure_to_enoji(&self) -> &str {
        const PA_TO_MM_MERCURY: f32 = 133.322;
        const NORMAL_PRESSURE: f32 = 101_325.0 / PA_TO_MM_MERCURY;

        if (self.pressure / PA_TO_MM_MERCURY) > (NORMAL_PRESSURE + 10.0) {
            "⬆️ повышенное давление"
        } else if (self.pressure / PA_TO_MM_MERCURY) < (NORMAL_PRESSURE - 10.0) {
            "⬇️ пониженное давление"
        } else {
            ""
        }
    }

    pub fn should_alert(&self) -> bool {
        self.temp > 30. || self.temp < 15. || self.humidity >= 85.
    }

    pub fn to_new_weather_message(&self) -> NewWeatherMessage {
        NewWeatherMessage::new(self.temp, self.humidity, self.humidity)
    }
}

impl NewWeatherMessage {
    pub fn new(tmp: f32, press: f32, hum: f32) -> NewWeatherMessage {
        NewWeatherMessage {
            timestamp: Utc::now().to_rfc3339(),
            temp: tmp,
            pressure: press,
            humidity: hum,
        }
    }

    pub fn save_to_db(&self, connection: &SqliteConnection) -> QueryResult<usize> {
        let result = diesel::insert_into(weather_log::table)
            .values(self)
            .execute(connection);
        
        result
    }
}

pub fn median_weather(n_days: usize, limit: i64, connection: &SqliteConnection) -> QueryResult<Vec<WeatherMessage>> {
    let weather_logs = weather_log.order(timestamp.desc()).limit(limit).load::<WeatherMessage>(connection);
    weather_logs
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_weather_message() {
        let connection = establish_connection("db.sqlite");
        let results = weather_log.load::<WeatherMessage>(&connection);
        assert!(results.is_ok())
    }

    #[test]
    fn test_insert_and_delete_weather_message() {
        let connection = establish_connection("db.sqlite");
        let new_weather_message = NewWeatherMessage::new(25.0, 10000.0, 55.0);
        let result = diesel::insert_into(weather_log::table)
            .values(&new_weather_message)
            .execute(&connection);

        assert!(result.is_ok());

        let last_log = weather_log
            .order(timestamp.desc())
            .load::<WeatherMessage>(&connection);
        assert!(last_log.is_ok());

        let last_log = &last_log.unwrap()[0];
        let del_result =
            diesel::delete(weather_log.filter(id.eq(last_log.id))).execute(&connection);
        assert!(del_result.is_ok());

        let results = weather_log.load::<WeatherMessage>(&connection);
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }
}
