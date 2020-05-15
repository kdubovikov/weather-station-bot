table! {
    subscribers (id) {
        id -> Integer,
        telegram_chat_id -> BigInt,
    }
}

table! {
    weather_log (id) {
        id -> Integer,
        timestamp -> Text,
        temp -> Float,
        pressure -> Float,
        humidity -> Float,
    }
}

allow_tables_to_appear_in_same_query!(
    subscribers,
    weather_log,
);
