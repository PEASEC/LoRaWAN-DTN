//! Runtime running the event loop and providing an interface to modify callbacks.

pub mod callbacks;
pub mod event_loop;

use crate::downlinks::{Downlink, DownlinkType};
use crate::error::{CallbackRemoveError, RuntimeError};
use crate::runtime::callbacks::{
    AllGatewaysCallbackStorage, CommandConfigCallback, CommandDownCallback, CommandExecCallback,
    CommandRawCallback, EventAckCallback, EventExecCallback, EventRawCallback, EventStatsCallback,
    EventUpCallback, StateConnCallback,
};
use callbacks::{CallbackDrawers, PerGatewayCallbackStorage};
use prost::Message;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, trace};
use uuid::Uuid;

/// Default ChirpStack event topic.
static EVENT_TOPIC: &str = "eu868/gateway/+/event/+";
/// Default ChirpStack command topic.
static COMMAND_TOPIC: &str = "eu868/gateway/+/command/+";
/// Default ChirpStack states topic.
static STATES_TOPIC: &str = "eu868/gateway/+/states/+";

/// Type to interact with the event loop of the MQTT client.
///
/// Add and remove callbacks, edit ignored gateways or send downlinks.
/// Don't drop the runtime as it stops the event loop.
#[derive(Debug, Clone)]
pub struct Runtime {
    /// Callbacks registered for specific gateways.
    per_gateway_callbacks: PerGatewayCallbackStorage,
    /// Callbacks registered for all gateways.
    all_gateways_callbacks: AllGatewaysCallbackStorage,
    /// MQTT client.
    mqtt_client: AsyncClient,
    /// Stop signal channel transceiver end. Used to signal the event loop to stop.
    stop_signal_tx: tokio::sync::mpsc::Sender<()>,
    /// Keeps track of whether the stop method of the runtime has been called.
    received_stop: bool,
}

impl Runtime {
    /// Create a new runtime with simplified parameters.
    #[tracing::instrument]
    pub async fn new(
        id: &str,
        host: &str,
        port: u16,
        connection_error_sender: Option<tokio::sync::broadcast::Sender<String>>,
    ) -> Result<Self, RuntimeError> {
        let mqtt_options = MqttOptions::new(id, host, port);
        Self::new_with_mqtt_options(mqtt_options, connection_error_sender).await
    }

    /// Create a new runtime with the supplied [`MqttOptions`].
    #[tracing::instrument]
    pub async fn new_with_mqtt_options(
        mqtt_options: MqttOptions,
        connection_error_sender: Option<tokio::sync::broadcast::Sender<String>>,
    ) -> Result<Self, RuntimeError> {
        info!("Connecting to {:?}", mqtt_options);
        let (mqtt_client, event_loop) = AsyncClient::new(mqtt_options, 10);
        let per_gateway_callbacks = Arc::new(RwLock::new(HashMap::new()));
        let per_gateway_callbacks_clone = per_gateway_callbacks.clone();
        let all_gateways_callbacks = Arc::new(RwLock::new(CallbackDrawers::new()));
        let all_gateways_callbacks_clone = all_gateways_callbacks.clone();
        let (stop_signal_tx, stop_signal_rx) = tokio::sync::mpsc::channel(1);
        info!("Spawning event loop");
        // spawn event loop task (tokio task)
        tokio::task::spawn(async move {
            event_loop::run_event_loop(
                event_loop,
                per_gateway_callbacks_clone,
                all_gateways_callbacks_clone,
                connection_error_sender,
                stop_signal_rx,
            )
            .await;
        });

        trace!("subscribing to {}", EVENT_TOPIC);
        mqtt_client.subscribe(EVENT_TOPIC, QoS::AtLeastOnce).await?;
        trace!("subscribing to {}", COMMAND_TOPIC);
        mqtt_client
            .subscribe(COMMAND_TOPIC, QoS::AtLeastOnce)
            .await?;
        trace!("subscribing to {}", STATES_TOPIC);
        mqtt_client
            .subscribe(STATES_TOPIC, QoS::AtLeastOnce)
            .await?;

        Ok(Runtime {
            per_gateway_callbacks,
            all_gateways_callbacks,
            mqtt_client,
            stop_signal_tx,
            received_stop: false,
        })
    }

    /// Add a callback for a downlink command.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every downlink command.
    #[tracing::instrument(skip(self))]
    pub async fn add_command_config_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn CommandConfigCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();
        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .command
                .config
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .command
                .config
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a downlink command.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every downlink command.
    #[tracing::instrument(skip(self))]
    pub async fn add_command_down_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn CommandDownCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();
        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .command
                .down
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .command
                .down
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a exec command.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every exec command.
    #[tracing::instrument(skip(self))]
    pub async fn add_command_exec_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn CommandExecCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();

        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .command
                .exec
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .command
                .exec
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a raw command.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every raw command.
    #[tracing::instrument(skip(self))]
    pub async fn add_command_raw_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn CommandRawCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();

        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .command
                .raw
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .command
                .raw
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a stats event.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every stats event.
    #[tracing::instrument(skip(self))]
    pub async fn add_event_stats_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn EventStatsCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();

        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .event
                .stats
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .event
                .stats
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a up event.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every up event.
    #[tracing::instrument(skip(self))]
    pub async fn add_event_up_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn EventUpCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();

        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .event
                .up
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .event
                .up
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a ack event.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every ack event.
    #[tracing::instrument(skip(self))]
    pub async fn add_event_ack_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn EventAckCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();

        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .event
                .ack
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .event
                .ack
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a exec event.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every exec event.
    #[tracing::instrument(skip(self))]
    pub async fn add_event_exec_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn EventExecCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();

        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .event
                .exec
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .event
                .exec
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a raw event.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every raw event.
    #[tracing::instrument(skip(self))]
    pub async fn add_event_raw_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn EventRawCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();
        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .event
                .raw
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .event
                .raw
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Add a callback for a conn state.
    /// If `gateway_id` is `Some(...)`, the callback is only applied the gateway topic, otherwise
    /// the callback is applied to every conn state.
    #[tracing::instrument(skip(self))]
    pub async fn add_state_conn_callback(
        &mut self,
        gateway_id: Option<String>,
        callback: Box<dyn StateConnCallback>,
    ) -> Result<Uuid, RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let uuid = Uuid::new_v4();
        if let Some(gateway_id) = gateway_id {
            let mut callbacks_lock = self.per_gateway_callbacks.write().await;
            let callback_drawers = callbacks_lock
                .entry(gateway_id)
                .or_insert_with(CallbackDrawers::new);

            if callback_drawers
                .state
                .conn
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        } else {
            let mut all_gateways_callbacks_lock = self.all_gateways_callbacks.write().await;
            if all_gateways_callbacks_lock
                .state
                .conn
                .insert(uuid, Arc::new(callback))
                .is_some()
            {
                Err(RuntimeError::UuidCollision)
            } else {
                Ok(uuid)
            }
        }
    }

    /// Remove all callbacks for the listed gateway IDs.
    #[tracing::instrument(skip(self))]
    pub async fn remove_callbacks_with_gateways(
        &self,
        gateway_ids: Vec<String>,
    ) -> Result<(), RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let mut callbacks = self.per_gateway_callbacks.write().await;
        for gateway_id in gateway_ids {
            callbacks.remove(&gateway_id);
        }
        Ok(())
    }

    /// Remove the callback with the supplied ID.
    #[tracing::instrument(skip(self))]
    pub async fn remove_callback(&self, uuid: Uuid) -> Result<(), RuntimeError> {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let mut per_gateway_callbacks = self.per_gateway_callbacks.write().await;
        let mut found = false;
        for (_, callback_drawer) in per_gateway_callbacks.iter_mut() {
            if callback_drawer.remove(&uuid).is_ok() {
                trace!("Found a callback to remove.");
                found = true;
            }
        }
        let mut all_gateways_callbacks = self.all_gateways_callbacks.write().await;
        if all_gateways_callbacks.remove(&uuid).is_ok() {
            found = true;
        }
        if found {
            Ok(())
        } else {
            trace!("No callback was found.");
            Err(CallbackRemoveError::NoSuchCallback { uuid }.into())
        }
    }

    /// Enqueues a downlink to be sent from the specified gateway.
    #[tracing::instrument(skip_all)]
    pub async fn enqueue<Dt>(
        &self,
        sender_gateway: &str,
        downlink: Downlink<Dt>,
    ) -> Result<(), RuntimeError>
    where
        chirpstack_api::gw::DownlinkFrame: From<Downlink<Dt>>,
        Dt: DownlinkType,
    {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let gateway_downlink_command_topic = format!("eu868/gateway/{sender_gateway}/command/down");
        let downlink_frame: chirpstack_api::gw::DownlinkFrame = downlink.into();
        let message = downlink_frame.encode_to_vec();

        trace!(
            "Sending {:?} to: {}",
            downlink_frame,
            gateway_downlink_command_topic
        );

        Ok(self
            .mqtt_client
            .publish(
                gateway_downlink_command_topic,
                QoS::AtMostOnce,
                false,
                message,
            )
            .await?)
    }

    /// Enqueues a downlink to be sent from the specified gateway.
    #[tracing::instrument(skip_all)]
    pub fn try_enqueue<Dt>(
        &self,
        sender_gateway: &str,
        downlink: Downlink<Dt>,
    ) -> Result<(), RuntimeError>
    where
        chirpstack_api::gw::DownlinkFrame: From<Downlink<Dt>>,
        Dt: DownlinkType,
    {
        if self.received_stop {
            return Err(RuntimeError::Stopped);
        }
        let gateway_downlink_command_topic = format!("eu868/gateway/{sender_gateway}/command/down");
        let downlink_frame: chirpstack_api::gw::DownlinkFrame = downlink.into();
        let message = downlink_frame.encode_to_vec();

        trace!(
            "Sending {:?} to: {}",
            downlink_frame,
            gateway_downlink_command_topic
        );

        Ok(self.mqtt_client.try_publish(
            gateway_downlink_command_topic,
            QoS::AtMostOnce,
            false,
            message,
        )?)
    }

    /// Stop the runtime.
    ///
    /// Sends a MQTT disconnect via the event loop and stops the event loop task afterwards.
    pub fn stop_event_loop(&mut self) {
        if let Err(err) = self.mqtt_client.try_disconnect() {
            error!(%err);
        };
        if let Err(err) = self.stop_signal_tx.try_send(()) {
            error!(%err);
        }
        self.received_stop = true;
    }
}
