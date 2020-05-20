CREATE TABLE weather_log (
    id INTEGER NOT NULL,
    timestamp TEXT NOT NULL,
    temp REAL NOT NULL,
    pressure REAL NOT NULL,
    humidity REAL NOT NULL,
    PRIMARY KEY(id)
);