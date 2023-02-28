//! Callback traits and callback storage implementations.

use crate::error::CallbackError;
use crate::gateway_topics::{CommandType, EventType, ParsedTopic, StateType, TopicType};
use async_trait::async_trait;
use core::fmt;
use prost::bytes::Bytes;
use prost::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Implement this trait if you want to build a down config callback.
#[async_trait]
pub trait CommandConfigCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_config_command(
        &self,
        gateway_id: String,
        config_command: chirpstack_api::gw::GatewayConfiguration,
    );
}

/// Implement this trait if you want to build a down command callback.
#[async_trait]
pub trait CommandDownCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_down_command(
        &self,
        gateway_id: String,
        downlink_command: chirpstack_api::gw::DownlinkFrame,
    );
}

/// Implement this trait if you want to build a exec command callback.
#[async_trait]
pub trait CommandExecCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_exec_command(
        &self,
        gateway_id: String,
        exec_command: chirpstack_api::gw::GatewayCommandExecRequest,
    );
}

/// Implement this trait if you want to build a raw command callback.
#[async_trait]
pub trait CommandRawCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_raw_command(
        &self,
        gateway_id: String,
        raw_command: chirpstack_api::gw::RawPacketForwarderCommand,
    );
}

/// Implement this trait if you want to build a stats event callback.
#[async_trait]
pub trait EventStatsCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_stats_event(
        &self,
        gateway_id: String,
        stats_event: chirpstack_api::gw::GatewayStats,
    );
}

/// Implement this trait if you want to build a up event callback.
#[async_trait]
pub trait EventUpCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_up_event(
        &self,
        gateway_id: String,
        up_event: chirpstack_api::gw::UplinkFrame,
    );
}

/// Implement this trait if you want to build a ack event callback.
#[async_trait]
pub trait EventAckCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_ack_event(
        &self,
        gateway_id: String,
        ack_event: chirpstack_api::gw::DownlinkTxAck,
    );
}

/// Implement this trait if you want to build a exec event callback.
#[async_trait]
pub trait EventExecCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_exec_event(
        &self,
        gateway_id: String,
        exec_event: chirpstack_api::gw::GatewayCommandExecResponse,
    );
}

/// Implement this trait if you want to build a raw event callback.
#[async_trait]
pub trait EventRawCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_raw_event(
        &self,
        gateway_id: String,
        raw_event: chirpstack_api::gw::RawPacketForwarderEvent,
    );
}

/// Implement this trait if you want to build a conn state callback.
#[async_trait]
pub trait StateConnCallback: Send + Sync + fmt::Debug {
    /// The function is called with every incoming message it was registered for.
    async fn dispatch_conn_state(
        &self,
        gateway_id: String,
        conn_state: chirpstack_api::gw::ConnState,
    );
}

/// Contains all callback drawers, is linked to a gateway id in the [`Runtime`](crate::runtime::Runtime).
#[derive(Debug)]
pub struct CallbackDrawers {
    /// Contains all command callbacks.
    pub(crate) command: CallbackCommandDrawer,
    /// Contains all event callbacks.
    pub(crate) event: CallbackEventDrawer,
    /// Contains all state callbacks.
    pub(crate) state: CallbackStateDrawer,
}

/// Contains all command callbacks.
#[derive(Debug)]
pub struct CallbackCommandDrawer {
    /// Config command callbacks.
    pub(crate) config: HashMap<Uuid, Arc<Box<dyn CommandConfigCallback>>>,
    /// Downlink command callbacks.
    pub(crate) down: HashMap<Uuid, Arc<Box<dyn CommandDownCallback>>>,
    /// Exec command callbacks.
    pub(crate) exec: HashMap<Uuid, Arc<Box<dyn CommandExecCallback>>>,
    /// Raw command callbacks.
    pub(crate) raw: HashMap<Uuid, Arc<Box<dyn CommandRawCallback>>>,
}

/// Contains all event callbacks.
#[derive(Debug)]
pub struct CallbackEventDrawer {
    /// Stats event callbacks.
    pub(crate) stats: HashMap<Uuid, Arc<Box<dyn EventStatsCallback>>>,
    /// Uplink event callbacks.
    pub(crate) up: HashMap<Uuid, Arc<Box<dyn EventUpCallback>>>,
    /// Ack event callbacks.
    pub(crate) ack: HashMap<Uuid, Arc<Box<dyn EventAckCallback>>>,
    /// Exec event callbacks.
    pub(crate) exec: HashMap<Uuid, Arc<Box<dyn EventExecCallback>>>,
    /// Raw event callbacks.
    pub(crate) raw: HashMap<Uuid, Arc<Box<dyn EventRawCallback>>>,
}

/// Contains all state callbacks.
#[derive(Debug)]
pub struct CallbackStateDrawer {
    /// Conn state callbacks.
    pub(crate) conn: HashMap<Uuid, Arc<Box<dyn StateConnCallback>>>,
}

impl CallbackDrawers {
    /// Create a new [`CallbackDrawers`] instance with empty [`CallbackCommandDrawer`],
    /// [`CallbackEventDrawer`] and [`CallbackStateDrawer`].
    pub(crate) fn new() -> Self {
        CallbackDrawers {
            command: CallbackCommandDrawer::new(),
            event: CallbackEventDrawer::new(),
            state: CallbackStateDrawer::new(),
        }
    }

    /// Remove the callback with the specified `uuid`.
    /// # Error
    /// Returns an error if the callback was not found.
    pub(crate) fn remove(&mut self, uuid: &Uuid) -> Result<(), CallbackError> {
        if self.command.remove(uuid).is_ok()
            | self.event.remove(uuid).is_ok()
            | self.state.remove(uuid).is_ok()
        {
            Ok(())
        } else {
            Err(CallbackError::NoSuchCallback { uuid: *uuid })
        }
    }

    #[tracing::instrument]
    pub(crate) async fn dispatch(
        &self,
        topic: ParsedTopic,
        msg_payload: Bytes,
    ) -> Result<(), CallbackError> {
        match topic.topic_type {
            TopicType::Event(event_type) => {
                self.event
                    .dispatch(event_type, topic.gateway_id, msg_payload)
                    .await?
            }
            TopicType::State(state_type) => {
                self.state
                    .dispatch(state_type, topic.gateway_id, msg_payload)
                    .await?
            }
            TopicType::Command(command_type) => {
                self.command
                    .dispatch(command_type, topic.gateway_id, msg_payload)
                    .await?
            }
        }
        Ok(())
    }
}

impl CallbackCommandDrawer {
    pub(crate) fn new() -> Self {
        CallbackCommandDrawer {
            config: HashMap::new(),
            down: HashMap::new(),
            exec: HashMap::new(),
            raw: HashMap::new(),
        }
    }

    pub(crate) fn remove(&mut self, uuid: &Uuid) -> Result<(), CallbackError> {
        if self.down.remove(uuid).is_some()
            | self.exec.remove(uuid).is_some()
            | self.raw.remove(uuid).is_some()
        {
            Ok(())
        } else {
            Err(CallbackError::NoSuchCallback { uuid: *uuid })
        }
    }

    pub(crate) async fn dispatch(
        &self,
        command_type: CommandType,
        gateway_id: String,
        msg_payload: Bytes,
    ) -> Result<(), CallbackError> {
        match command_type {
            CommandType::Down => {
                let downlink_frame = chirpstack_api::gw::DownlinkFrame::decode(msg_payload)?;
                for callback_fn in self.down.values() {
                    let downlink_frame_clone = downlink_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_down_command(gateway_id_clone, downlink_frame_clone)
                            .await;
                    });
                }
            }
            CommandType::Config => {
                let config_frame = chirpstack_api::gw::GatewayConfiguration::decode(msg_payload)?;
                for callback_fn in self.config.values() {
                    let config_frame_clone = config_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_config_command(gateway_id_clone, config_frame_clone)
                            .await;
                    });
                }
            }
            CommandType::Exec => {
                let exec_frame =
                    chirpstack_api::gw::GatewayCommandExecRequest::decode(msg_payload)?;
                for callback_fn in self.exec.values() {
                    let exec_frame_clone = exec_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_exec_command(gateway_id_clone, exec_frame_clone)
                            .await;
                    });
                }
            }
            CommandType::Raw => {
                let raw_frame = chirpstack_api::gw::RawPacketForwarderCommand::decode(msg_payload)?;
                for callback_fn in self.raw.values() {
                    let raw_frame_clone = raw_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_raw_command(gateway_id_clone, raw_frame_clone)
                            .await;
                    });
                }
            }
        }
        Ok(())
    }
}

impl CallbackEventDrawer {
    pub(crate) fn new() -> Self {
        CallbackEventDrawer {
            stats: HashMap::new(),
            up: HashMap::new(),
            ack: HashMap::new(),
            exec: HashMap::new(),
            raw: HashMap::new(),
        }
    }

    pub(crate) fn remove(&mut self, uuid: &Uuid) -> Result<(), CallbackError> {
        if self.stats.remove(uuid).is_some()
            | self.up.remove(uuid).is_some()
            | self.ack.remove(uuid).is_some()
            | self.exec.remove(uuid).is_some()
            | self.raw.remove(uuid).is_some()
        {
            Ok(())
        } else {
            Err(CallbackError::NoSuchCallback { uuid: *uuid })
        }
    }

    pub(crate) async fn dispatch(
        &self,
        event_type: EventType,
        gateway_id: String,
        msg_payload: Bytes,
    ) -> Result<(), CallbackError> {
        match event_type {
            EventType::Stats => {
                let gateway_stats = chirpstack_api::gw::GatewayStats::decode(msg_payload)?;
                for callback_fn in self.stats.values() {
                    let gateway_stats_clone = gateway_stats.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_stats_event(gateway_id_clone, gateway_stats_clone)
                            .await;
                    });
                }
            }
            EventType::Up => {
                let uplink_frame = chirpstack_api::gw::UplinkFrame::decode(msg_payload)?;
                for callback_fn in self.up.values() {
                    let uplink_frame_clone = uplink_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_up_event(gateway_id_clone, uplink_frame_clone)
                            .await;
                    });
                }
            }
            EventType::Ack => {
                let ack_frame = chirpstack_api::gw::DownlinkTxAck::decode(msg_payload)?;
                for callback_fn in self.ack.values() {
                    let ack_frame_clone = ack_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_ack_event(gateway_id_clone, ack_frame_clone)
                            .await;
                    });
                }
            }
            EventType::Exec => {
                let exec_frame =
                    chirpstack_api::gw::GatewayCommandExecResponse::decode(msg_payload)?;
                for callback_fn in self.exec.values() {
                    let exec_frame_clone = exec_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_exec_event(gateway_id_clone, exec_frame_clone)
                            .await;
                    });
                }
            }
            EventType::Raw => {
                let raw_frame = chirpstack_api::gw::RawPacketForwarderEvent::decode(msg_payload)?;
                for callback_fn in self.raw.values() {
                    let raw_frame_clone = raw_frame.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_raw_event(gateway_id_clone, raw_frame_clone)
                            .await;
                    });
                }
            }
        }
        Ok(())
    }
}

impl CallbackStateDrawer {
    pub(crate) fn new() -> Self {
        CallbackStateDrawer {
            conn: HashMap::new(),
        }
    }

    pub(crate) fn remove(&mut self, uuid: &Uuid) -> Result<(), CallbackError> {
        if self.conn.remove(uuid).is_some() {
            Ok(())
        } else {
            Err(CallbackError::NoSuchCallback { uuid: *uuid })
        }
    }

    pub(crate) async fn dispatch(
        &self,
        state_type: StateType,
        gateway_id: String,
        msg_payload: Bytes,
    ) -> Result<(), CallbackError> {
        match state_type {
            StateType::Conn => {
                let conn_state = chirpstack_api::gw::ConnState::decode(msg_payload)?;
                for callback_fn in self.conn.values() {
                    let conn_state_clone = conn_state.clone();
                    let gateway_id_clone = gateway_id.clone();
                    let callback_fn_clone = callback_fn.clone();
                    tokio::task::spawn(async move {
                        callback_fn_clone
                            .dispatch_conn_state(gateway_id_clone, conn_state_clone)
                            .await;
                    });
                }
            }
        }
        Ok(())
    }
}

/// Thread safe callback storage for the [`Runtime`](crate::runtime::Runtime).
pub(crate) type PerGatewayCallbackStorage = Arc<RwLock<HashMap<String, CallbackDrawers>>>;
/// Thread safe callback storage for the [`Runtime`](crate::runtime::Runtime).
pub(crate) type AllGatewaysCallbackStorage = Arc<RwLock<CallbackDrawers>>;
