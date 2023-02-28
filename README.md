# LoRaWAN DTN

This project aims to provide an interface for DTN based multi-hop communication to other applications. It exploits an existing Chirpstack setup and integrates into the internal MQTT broker for transceiving messages via LoRaWAN gateways.

This setup requires at least two Gateway setups (in LoRaWAN range) to be functional, as it is intended to exchange data between such setups in times of network outages.

## Submodules

* [chirpstack_api_wrapper](./chirpstack_api_wrapper)
  * API Wrapper for ChirpStack REST API
  
* [chirpstack_gwb_integration](./chirpstack_gwb_integration)
  * library that handles mosquitto based communication with a running ChirpStack

* [chirpstack_gwb_integration_cli](./chirpstack_gwb_integration_cli)
  * CLI for send/receive data via a LoRaWAN Gateway

* [spatz](./spatz)
  * backend that sends/receives DTN/BP7 messages via LoRaWAN-Gateways


## Acknowledgments
* This work was created at Science and Technology for Peace and Security (PEASEC), Technical University of Darmstadt, www.peasec.de, and supported by funds of the German Government’s Special Purpose Fund held at Landwirtschaftliche Rentenbank in the projects Geobox-II and AgriRegio.
  * Contributors under those funds:
    * Julian Schindel
    * Franz Kuntke


## License
Sub-projects of this repository are licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
