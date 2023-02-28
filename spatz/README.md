# Spatz 🐦

A daemon integrating the custom HofBox LoRaWAN protocol into ChirpStack applications via the ChirpStack gateway bridge.

## Config

```toml
# Connection information for the ChirpStack API
[chirpstack_api]
# ChirpStack API token (generated in the web interface)
api_token="abcd"
# Tenenat ID if available, leave empty ("") for admin usage
tenant_id="abcd"
# ChirpStack URL and port
url="http://127.0.0.1"
port=8080

# MQTT configuration
[mqtt]
# MQTT broker URL and port
url="127.0.0.1"
port=1883
# Client ID identifies the Spatz daemon to the MQTT broker
client_id="spatz-daemon"

[daemon]
# The address and port the Spatz daemon shoul bind to
bind_addr="127.0.0.1"
bind_port=3000
# List of default end device IDs
end_device_ids=["1234567890", "0987654321"]

# Message cache config, the message cache keeps track of what messages have already been sent/seen
[daemon.message_cache]
# Timeout after which the message is considered new again
timeout_minutes=30
# Iterval at which the cache entries are checked for expiry
cleanup_interval_seconds=30
# Whether to reset the timeout back to the initial amount if the message is seen again
reset_timeout=false

# Configuration for the send manager
[daemon.send_config]
# At what interval should messages be sent (seconds)
periodic_send_delay=5
# Size of the relay queue, hold single messages received via LoRaWAN uplink
relay_queue_size=10
# Bundle send buffer queue, hold whole bundles
bundle_queue_size=10
# Announcement send buffer queue, holds whole announcements
announcement_queue_size=10
```

## Usage
The daemon expects a file path to a config file as described above.
```
./spatz --config-file-path path/to/file
```

## API
The OpenAPI spec for Spatz is hosted at `/api.json`.

## Debugging
### API
List end device IDs
```shell
curl 127.0.0.1:3000/api/end_devices
```
Add end device IDs
```shell
curl -X POST -H 'Content-Type: application/json' -d '{"end_devices": ["1","2","3","4"]}' 127.0.0.1:3000/api/end_devices
```
Remove end device IDs
```shell
curl -X DELETE -H 'Content-Type: application/json' -d '{"end_devices": ["1","2","3","4"]}' 127.0.0.1:3000/api/end_devices
```


## Acknowledgments
* This work was created at Science and Technology for Peace and Security (PEASEC), Technical University of Darmstadt, www.peasec.de, and supported by funds of the German Government’s Special Purpose Fund held at Landwirtschaftliche Rentenbank in the projects Geobox-II and AgriRegio.
  * Contributors under those funds:
    * Julian Schindel
    * Franz Kuntke

## License
Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in `spatz` by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
