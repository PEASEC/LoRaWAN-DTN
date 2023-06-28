//! Configuration types.

use clap::Parser;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Configuration of the daemon application.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    /// ChirpStack API credentials and parameters.
    pub chirpstack_api: ChirpStackApiConfig,
    /// MQTT connection configuration
    pub mqtt: MqttConfig,
    /// Daemon configuration
    pub daemon: DaemonConfig,
}

/// ChirpStack API credentials and parameters.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChirpStackApiConfig {
    /// ChirpStack URL
    pub url: String,
    /// ChirpStack port
    pub port: u16,
    /// ChirpStack API token
    pub api_token: String,
    /// ChirpStack Tenant ID, None if used as admin
    pub tenant_id: Option<String>,
}

/// MQTT connection configuration
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MqttConfig {
    /// MQTT URL
    pub url: String,
    /// MQTT port
    pub port: u16,
    /// MQTT client ID
    pub client_id: String,
}

/// Daemon configuration
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Bind configuration
    pub bind_config: BindConfig,
    /// End device ID
    ///
    /// Identification number inside the emergency LoRaWAN network
    pub end_device_ids: Vec<String>,
    /// Send configuration
    pub queue_config: QueueConfig,
    /// Configuration of the packet cache
    pub packet_cache: PacketCacheConfig,
    /// Configuration of the routing algorithm its parameters
    pub routing_algorithm_config: RoutingAlgorithmConfig,
    /// Path to SQLITE database file
    pub db_path: Option<String>,
}

/// Bind configuration
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BindConfig {
    /// Address to bind to
    pub bind_addr: IpAddr,
    /// Port to bind to
    pub bind_port: u16,
}

/// Send configuration
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QueueConfig {
    /// Max amount of queued relay packet.
    pub relay_queue_size: usize,
    /// Max amount of queued bundles.
    pub bundle_queue_size: usize,
    /// Max amount of queued announcements.
    pub announcement_queue_size: usize,
}

/// Configuration for routing algorithms
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum RoutingAlgorithmConfig {
    /// Configuration for the flooding routing algorithm
    Flooding(FloodingConfig),
}

/// Flooding routing algorithm configuration
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FloodingConfig {
    /// Delay between send attempts in seconds.
    pub periodic_send_delay: u64,
}

/// Message Cache configuration
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PacketCacheConfig {
    /// The timeout for which the same packet is ignored. In minutes.
    pub timeout_minutes: u32,
    /// The interval at which expired entries are removed from the cache.
    pub cleanup_interval_seconds: u64,
    /// Whether the timeout is reset if the same packet is seen again while the timeout has not
    /// elapsed.
    pub reset_timeout: bool,
}

/// CLI parameters.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct CliParameters {
    /// Path to config file
    #[clap(long, value_parser, default_value = "./config/default.toml")]
    pub config_file_path: String,

    /// Path to sqlite DB file
    #[clap(long, value_parser, default_value = "sqlite://spatz_db.sqlite")]
    pub db_url: String,
}
