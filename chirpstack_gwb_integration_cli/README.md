# ChirpStack GatewayBridge Integration Client

A small cli tool for sending/receiving data through a ChirpStack-enabled LoRaWAN-Gateway.

## Usage
The daemon expects a file path to a config file (same config file format as used by [Spatz](../spatz)).

Alternatively you can also specify the following options directly via the CLI:
* `--api-token "API_TOKEN"` (set ChirpStack API Token)
* `--tenant-id "TENANT_ID"` (set ChirpStack Tenant ID)
* `--chirpstack_url "URL"`  (set URL of ChirpStack, e.g. `127.0.0.1`)
* `--chirpstack_port "PORT"` (set HTTP-Port of ChirpStack, e.g. `8080`)
* `--mqtt_url "URL"` (set URL of MQTT, e.g. `127.0.0.1`)
* `--mqtt_port "PORT"` (set port of MQTT, e.g. `1883`)


Example commands for both modes, listening (receiving) and downlink (sending) are as follows:

### Listening

```
cargo run -- --config-file config/config_file.toml listening 
```
The `listening` subcommand allows following options:

* `--prefix NUMBER` (allow to filter incoming messages, based on first payload byte, NUMBER must be between 0 and 255)
* `--verbose` (enable verbose mode)


### Downlink

```
cargo run -- --config-file config/config_file.toml downlink --payload "Test" --spreading-factor=7 --prefix 224
```

The `downlink` subcommand allows following options:


* `--frequency NUMBER` (set frequency, NUMBER must be 868100000, 868300000, or 868500000)
* `--bandwidth NUMBER` (set bandwidth, NUMBER must be 125000 or 250000)
* `--spreading_factor NUMBER` (set spreading factor, must be between 7 and 12)
* `--data_rate NUMBER` (set data rate and overwrites frequency and spreading factor settings, NUMBER must be between 0 and 6)
* `--payload "STRING"` (set payload)
* `--network_id` (set network ID)
* `--prefix NUMBER` (allow to set a prefix byte, NUMBER must be between 0 and 255)
* `--verbose` (enable verbose mode)


## Acknowledgments
* This work was created at Science and Technology for Peace and Security (PEASEC), Technical University of Darmstadt, www.peasec.de, and supported by funds of the German Government’s Special Purpose Fund held at Landwirtschaftliche Rentenbank in the projects Geobox-II and AgriRegio.
  * Contributors under those funds:
    * Julian Schindel
    * Franz Kuntke

## License
Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in `chirpstack_gwb_integration_cli` by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
