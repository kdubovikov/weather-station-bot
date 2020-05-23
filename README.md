# TODO
- [x] Add subscription command support
- [] Add Actix or Rocket API. For actix look here: https://turreta.com/2019/09/21/rest-api-with-rust-actix-web-and-postgresql-part-3/
- [] Fix weather message formatiing. Table columns do not align

# Cross-compiling for lunux
This project uses `cross` for cross-compilation. To cross-compile for linux:

```
docker build -t weather_station_bot_build ./docker/
cross build --target=x86_64-unknown-linux-musl --release
```

# Running database tests
```
diesel --database-url test.sqlite database setup
cargo test -- --test-threads=1
```