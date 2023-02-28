use clap::Parser;
use serde::Deserialize;
use std::net::IpAddr;

/// Configuration of the daemon application.
#[derive(Debug, Deserialize)]
pub struct Configuration {
    /// ChirpStack API credentials and parameters.
    pub chirpstack_api: ChirpStackApiConfig,
    /// MQTT connection configuration
    pub mqtt: MqttConfig,
    /// Daemon configuration
    pub daemon: DaemonConfig,
}

/// ChirpStack API credentials and parameters.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    /// MQTT URL
    pub url: String,
    /// MQTT port
    pub port: u16,
    /// MQTT client ID
    pub client_id: String,
}

/// Daemon configuration
#[derive(Debug, Deserialize)]
pub struct DaemonConfig {
    /// Address to bind to
    pub bind_addr: IpAddr,
    /// Port to bind to
    pub bind_port: u16,
    /// End device ID
    ///
    /// Identification number inside the emergency LoRaWAN network
    pub end_device_ids: Vec<String>,
    /// Send configuration
    pub send_config: SendConfig,
    /// Configuration of the message cache
    pub message_cache: MessageCacheConfig,
}

/// Send configuration
#[derive(Debug, Deserialize)]
pub struct SendConfig {
    /// Delay between send attempts in seconds.
    pub periodic_send_delay: u64,
    /// Max amount of queued relay messages.
    pub relay_queue_size: usize,
    /// Max amount of queued bundles.
    pub bundle_queue_size: usize,
    /// Max amount of queued announcements.
    pub announcement_queue_size: usize,
}

/// Message Cache configuration
#[derive(Debug, Deserialize)]
pub struct MessageCacheConfig {
    /// The timeout for which the same message is ignored. In minutes.
    pub timeout_minutes: u32,
    /// The interval at which expired entries are removed from the cache.
    pub cleanup_interval_seconds: u64,
    /// Whether the timeout is reset if the same message is seen again while the timeout has not
    /// elapsed.
    pub reset_timeout: bool,
}

/// CLI parameters.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct CliParameters {
    /// Path to config file
    #[clap(long, value_parser)]
    pub config_file_path: String,
}
