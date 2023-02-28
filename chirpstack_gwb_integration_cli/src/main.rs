use std::fs::File;
use std::io::Read;
use std::process;

use async_trait::async_trait;

use chirpstack_api_wrapper::ChirpStackApi;
use chirpstack_gwb_integration::downlinks;
use chirpstack_gwb_integration::downlinks::downlink_builder::DownlinkBuilder;
use chirpstack_gwb_integration::downlinks::downlink_item_builder::DownlinkItemBuilder;
use chirpstack_gwb_integration::downlinks::predefined_parameters::{
    Bandwidth, DataRate, Frequency, SpreadingFactor,
};
use chirpstack_gwb_integration::runtime::callbacks::EventUpCallback;
use chirpstack_gwb_integration::runtime::Runtime;

use chrono::Utc;
use clap::{Parser, Subcommand};
use rand::Rng;
use rumqttc::MqttOptions;
use serde_derive::Deserialize;
use tracing::error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Deserialize, Clone)]
struct Config {
    api_token: Option<String>,
    tenant_id: Option<String>,
    chirpstack_url: Option<String>,
    chirpstack_port: Option<u16>,
    mqtt_url: Option<String>,
    mqtt_port: Option<u16>,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Chirpstack API Token
    #[clap(short, long, value_parser)]
    api_token: Option<String>,

    /// Chirpstack tenant ID
    #[clap(short, long, value_parser)]
    tenant_id: Option<String>,

    /// Chirpstack URL
    #[clap(short, long, value_parser)]
    chirpstack_url: Option<String>,

    /// Chirpstack port
    #[clap(long, value_parser)]
    chirpstack_port: Option<u16>,

    /// MQTT URL
    #[clap(short, long, value_parser)]
    mqtt_url: Option<String>,

    /// MQTT port
    #[clap(long, value_parser)]
    mqtt_port: Option<u16>,

    /// Config file
    #[clap(long, value_parser)]
    config_file: Option<String>,

    /// Used for printing gateway's incoming frames or sending a downlink
    #[clap(subcommand)]
    subcommand: Option<Subcommands>,
}

#[derive(Subcommand, Debug)]
enum Subcommands {
    /// Does listening things
    Listening {
        /// Set verbose mode
        #[clap(short, long, action)]
        verbose: bool,

        /// Prefix byte value for payload (e.g. 224 for "proprietary lorawan payload")
        #[clap(long, value_parser)]
        prefix: Option<u8>,
    },

    /// Does downlink things
    Downlink {
        /// Set verbose mode
        #[clap(short, long, action, default_value_t = false)]
        verbose: bool,

        /// Frequency (868100000, 868300000, 868500000)
        #[clap(short, long, value_parser)]
        frequency: Option<u32>,

        /// Bandwidth (125000 or 250000)
        #[clap(short, long, value_parser)]
        bandwidth: Option<u32>,

        /// Spreading Factor (7..12)
        #[clap(short, long, value_parser)]
        spreading_factor: Option<u8>,

        /// Data Rate (0..6; overwrites frequency and spreading factor)
        #[clap(short, long, value_parser)]
        data_rate: Option<u8>,

        /// Payload
        #[clap(short, long, value_parser)]
        payload: String,

        /// Prefix byte value for payload (e.g. 224 for "proprietary lorawan payload")
        #[clap(long, value_parser)]
        prefix: Option<u8>,

        /// Add a network id to the downlink (intended for testing the daemon impl)
        #[clap(long, value_parser)]
        network_id: Option<u32>,
    },
}

#[tokio::main]
async fn listening(_verbose: &bool, config: Config, prefix: &Option<u8>) {
    let chirpstack_api = ChirpStackApi {
        url: config.chirpstack_url.unwrap(),
        port: config.chirpstack_port.unwrap(),
        api_token: config.api_token.unwrap(),
        tenant_id: config.tenant_id,
    };

    let gateway_ids = chirpstack_api.request_gateway_ids(100).await.unwrap();

    let mqtt_options = MqttOptions::new(
        "chi_bri_add_on_cli_listening",
        config.mqtt_url.unwrap(),
        config.mqtt_port.unwrap(),
    );
    let gateway_id = gateway_ids.iter().next().unwrap().clone();
    let mut runtime = Runtime::new_with_mqtt_options(mqtt_options).await.unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(100);
    let my_callback = Box::new(UplinkCallback { sender });
    runtime
        .add_event_up_callback(Some(gateway_id.clone()), my_callback)
        .await
        .unwrap();

    while let Some((_, up_event)) = receiver.recv().await {
        let dt = Utc::now();
        let timestamp: i64 = dt.timestamp();

        if !up_event.phy_payload.is_empty() {
            if prefix.is_some() {
                if up_event.phy_payload[0] == prefix.unwrap() {
                    let phy_payload_trimmed = &up_event.phy_payload.clone()[1..];
                    let payload_str = String::from_utf8(phy_payload_trimmed.to_vec());
                    if payload_str.is_ok() {
                        println!(
                            "{}: Payload (utf8) = {} | raw = {:?}",
                            timestamp,
                            payload_str.unwrap(),
                            up_event.phy_payload
                        );
                    } else {
                        println!("{}: Payload (raw) = {:?}", timestamp, up_event.phy_payload);
                    }
                } else {
                    println!("{}: Payload (raw) = {:?}", timestamp, up_event.phy_payload);
                }
            } else {
                let payload_str = String::from_utf8(up_event.phy_payload.clone());
                if payload_str.is_ok() {
                    println!(
                        "{}: Payload (utf8) = {} | raw = {:?}",
                        timestamp,
                        payload_str.unwrap(),
                        up_event.phy_payload
                    );
                } else {
                    println!("{}: Payload (raw) = {:?}", timestamp, up_event.phy_payload);
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct UplinkCallback {
    sender: tokio::sync::mpsc::Sender<(String, chirpstack_api::gw::UplinkFrame)>,
}

#[async_trait]
impl EventUpCallback for UplinkCallback {
    async fn dispatch_up_event(
        &self,
        gateway_id: String,
        up_event: chirpstack_api::gw::UplinkFrame,
    ) {
        self.sender.send((gateway_id, up_event)).await.unwrap()
    }
}

#[allow(clippy::too_many_arguments)]
#[tokio::main]
async fn downlink(
    _verbose: &bool,
    config: Config,
    frequency: &Option<u32>,
    bandwidth: &Option<u32>,
    spreading_factor: &Option<u8>,
    data_rate: &Option<u8>,
    payload: &String,
    prefix: &Option<u8>,
    network_id: &Option<u32>,
) {
    println!("In downlink");

    let chirpstack_api = ChirpStackApi {
        url: config.chirpstack_url.unwrap(),
        port: config.chirpstack_port.unwrap(),
        api_token: config.api_token.unwrap(),
        tenant_id: config.tenant_id,
    };

    let mqtt_options = MqttOptions::new(
        "chi_bri_add_on_cli_downlink",
        config.mqtt_url.unwrap(),
        config.mqtt_port.unwrap(),
    );

    let freq = match frequency {
        Some(f) => match f {
            868100000 => Frequency::Freq868_1,
            868300000 => Frequency::Freq868_3,
            868500000 => Frequency::Freq868_5,
            _ => {
                println!("Could not find \"frequency {}\", use default 868300000", f);
                Frequency::Freq868_3
            }
        },
        None => {
            println!("Using default frequency 868300000");
            Frequency::Freq868_3
        }
    };

    let dr = match data_rate {
        Some(d) => match d {
            0 => Some(DataRate::Eu863_870Dr0),
            1 => Some(DataRate::Eu863_870Dr1),
            2 => Some(DataRate::Eu863_870Dr2),
            3 => Some(DataRate::Eu863_870Dr3),
            4 => Some(DataRate::Eu863_870Dr4),
            5 => Some(DataRate::Eu863_870Dr5),
            6 => Some(DataRate::Eu863_870Dr6),
            _ => {
                println!("Could not find \"Data Rate {}\"", d);
                None
            }
        },
        None => {
            println!("Using default Data Rate 0");
            None
        }
    };

    let sf = spreading_factor.unwrap_or(12);
    let bw = bandwidth.unwrap_or(125000);

    let gateway_ids = chirpstack_api.request_gateway_ids(100).await.unwrap();

    let gateway_id = gateway_ids.iter().next().unwrap().clone();
    let mut runtime = Runtime::new_with_mqtt_options(mqtt_options).await.unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(100);
    let my_callback = Box::new(UplinkCallback { sender });
    runtime
        .add_event_up_callback(Some(gateway_id.clone()), my_callback)
        .await
        .unwrap();

    let mut pl_bytes = Vec::<u8>::new();
    //= payload.clone();

    if prefix.is_some() {
        pl_bytes.push(prefix.unwrap());
    }

    if network_id.is_some() {
        let network_id_bytes = network_id.unwrap().to_be_bytes();
        for network_id_byte in network_id_bytes {
            pl_bytes.push(network_id_byte)
        }
    }

    pl_bytes.extend_from_slice(payload.as_bytes());

    let mut item_builder = DownlinkItemBuilder::<downlinks::ImmediatelyClassC>::new()
        .phy_payload(pl_bytes)
        //.phy_payload(vec![0xff; 10])
        //.phy_payload("RAK7268-2".as_bytes())
        .frequency(freq)
        .power(14);
    if let Some(dr) = dr {
        item_builder = item_builder.data_rate(dr);
    } else {
        item_builder
            .raw_spreading_factor(SpreadingFactor::try_from(sf as u32).unwrap())
            .raw_bandwidth(Bandwidth::try_from_hz(bw).unwrap());
    }
    let item = item_builder.board(0).antenna(0).build().unwrap();
    let downlink = DownlinkBuilder::new()
        .gateway_id(gateway_id.clone())
        .downlink_id(rand::thread_rng().gen())
        .add_item(item)
        .build()
        .unwrap();

    tokio::spawn(async move {
        println!("Before enqueue");
        runtime.enqueue(&gateway_id, downlink).await.unwrap();
    });

    receiver.recv().await;

    let dt = Utc::now();
    let timestamp: i64 = dt.timestamp();

    let pre = if prefix.is_some() {
        prefix.unwrap().to_string()
    } else {
        "".to_string()
    };
    println!(
        "{}: sent downlink with payload \"{}\", prefix \"{}\"",
        timestamp, payload, pre
    );
}

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| {
                "chi_bri_add_on_cli=trace,chirpstack_gwb_integration=trace".into()
            }),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    let mut config = Config {
        api_token: cli.api_token,
        tenant_id: cli.tenant_id,
        chirpstack_url: cli.chirpstack_url,
        chirpstack_port: cli.chirpstack_port,
        mqtt_url: cli.mqtt_url,
        mqtt_port: cli.mqtt_port,
    };

    if let Some(c) = cli.config_file {
        let file = File::open(c);
        match file {
            Ok(mut f) => {
                let mut value = String::new();
                f.read_to_string(&mut value)
                    .expect("Error reading conf file");
                let config_file: Config = toml::from_str(&value).unwrap();

                if config.api_token.is_none() {
                    config.api_token = config_file.clone().api_token;
                    if config.api_token.is_none() {
                        error!("Missing api token!");
                        process::exit(1);
                    }
                }
                if config.tenant_id.is_none() {
                    config.tenant_id = config_file.clone().tenant_id;
                    if config.tenant_id.is_none() {
                        error!("Missing tenant id!");
                        process::exit(1);
                    }
                }
                if config.chirpstack_url.is_none() {
                    config.chirpstack_url = config_file.chirpstack_url;
                    if config.chirpstack_url.is_none() {
                        error!("Missing chirpstack url!");
                        process::exit(2);
                    }
                }
                if config.chirpstack_port.is_none() {
                    config.chirpstack_port = config_file.chirpstack_port;
                    if config.chirpstack_port.is_none() {
                        error!("Missing chirpstack port!");
                        process::exit(3);
                    }
                }
                if config.mqtt_url.is_none() {
                    config.mqtt_url = config_file.mqtt_url;
                    if config.mqtt_url.is_none() {
                        error!("Missing mqtt url!");
                        process::exit(4);
                    }
                }
                if config.mqtt_port.is_none() {
                    config.mqtt_port = config_file.mqtt_port;
                    if config.mqtt_port.is_none() {
                        error!("Missing mqtt port!");
                        process::exit(5);
                    }
                }
            }
            Err(_) => println!("Error reading file"),
        }
    } else {
        println!("No config file given");
    }

    // You can check the value provided by positional arguments, or option arguments
    println!("Use api_token: {}", config.api_token.clone().unwrap());

    match &cli.subcommand {
        Some(Subcommands::Listening { verbose, prefix }) => {
            println!(
                "'listening' with verbose set to: {:?}\n\t prefix = {:?}",
                verbose, prefix
            );
            listening(verbose, config, prefix);
        }
        Some(Subcommands::Downlink {
            verbose,
            frequency,
            bandwidth,
            spreading_factor,
            data_rate,
            payload,
            prefix,
            network_id,
        }) => {
            println!("'downlink' with verbose set to: {:?}\n\t frequency = {:?}\n\t bandwidth = {:?}\n\t spreading_factor = {:?}\n\t data_rate = {:?}\n\t payload = {}\n\t prefix = {:?}", verbose, frequency, bandwidth, spreading_factor, data_rate, payload, prefix);
            downlink(
                verbose,
                config,
                frequency,
                bandwidth,
                spreading_factor,
                data_rate,
                payload,
                prefix,
                network_id,
            );
        }
        _ => {
            println!("Please specify a subcommand!")
        }
    }
}
