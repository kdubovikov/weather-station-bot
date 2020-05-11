# TODO
- [] Add subscription command support
- [] Fix weather message formatiing. Table columns do not align

# Cross-compiling for lunux
This project uses `cross` for cross-compilation. To cross-compile for linux:

```
docker build -t weather_station_bot_build ./docker/
cross build --target=x86_64-unknown-linux-musl --release
```