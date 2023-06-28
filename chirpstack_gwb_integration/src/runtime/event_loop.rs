//! The event loop processing incoming MQTT messages.

use crate::gateway_topics::ParsedTopic;
use crate::runtime::callbacks::{AllGatewaysCallbackStorage, PerGatewayCallbackStorage};
use prost::Message;
use rumqttc::{Event, EventLoop, Incoming, Publish};
use std::time::Duration;
use tokio::time::Instant;
#[cfg(debug_assertions)]
use tracing::debug;
use tracing::{error, trace};

/// Runs the event loop processing incoming MQTT messages.
///
/// Needs to be spawned in an async task and kept running continuously.
#[tracing::instrument(skip_all)]
pub(crate) async fn run_event_loop(
    mut event_loop: EventLoop,
    per_gateway_callbacks: PerGatewayCallbackStorage,
    all_gateways_callbacks: AllGatewaysCallbackStorage,
    connection_error_sender: Option<tokio::sync::broadcast::Sender<String>>,
    mut stop_signal_rx: tokio::sync::mpsc::Receiver<()>,
) {
    let mut error_counter = 0;
    let mut last_error = Instant::now();
    loop {
        let notification = tokio::select! {
            _ = stop_signal_rx.recv() => {return},
            notification = event_loop.poll() => {notification}
        };

        match notification {
            Ok(notification) => {
                if let Event::Incoming(Incoming::Publish(pub_msg)) = notification {
                    trace!("Incoming msg Publish: {:?}", pub_msg);

                    #[cfg(debug_assertions)]
                    {
                        debug_printing(&pub_msg);
                    }

                    let parsed_topic = match ParsedTopic::try_from(pub_msg.topic.as_str()) {
                        Ok(parsed_topic) => parsed_topic,
                        Err(e) => {
                            error!(%e);
                            continue;
                        }
                    };

                    if let Some(per_gateway_callback_drawers) = per_gateway_callbacks
                        .read()
                        .await
                        .get(&parsed_topic.gateway_id)
                    {
                        trace!("Per gateway callback for message found.");
                        if let Err(e) = per_gateway_callback_drawers
                            .dispatch(parsed_topic.clone(), pub_msg.payload.clone())
                            .await
                        {
                            error!(%e);
                        }
                    }

                    if let Err(e) = all_gateways_callbacks
                        .read()
                        .await
                        .dispatch(parsed_topic, pub_msg.payload)
                        .await
                    {
                        error!(%e);
                    }
                }
            }
            Err(e) => {
                // The event loop tries to reconnect on it's own.
                // Connection error handling goes here if required.

                error!(%e);

                // If the last error happened over 30 seconds ago, reset error timer, otherwise
                // increase error counter.
                if Instant::now().duration_since(last_error) > Duration::from_secs(30) {
                    last_error = Instant::now();
                    error_counter = 1;
                } else {
                    error_counter += 1;
                }

                if error_counter >= 3 {
                    if let Some(connection_error_sender) = &connection_error_sender {
                        if connection_error_sender.receiver_count() > 0 {
                            if let Err(e) = connection_error_sender.send(e.to_string()) {
                                error!(%e);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Prints every published message received by the client.
///
/// Only included in debug builds.
#[cfg(debug_assertions)]
fn debug_printing(pub_msg: &Publish) {
    {
        if pub_msg.topic.contains("command") && pub_msg.topic.contains("down") {
            debug!(
                "Command down frame payload: {:?}",
                chirpstack_api::gw::DownlinkFrame::decode(pub_msg.payload.clone()).expect("Debug")
            );
        }
        if pub_msg.topic.contains("command") && pub_msg.topic.contains("exec") {
            debug!(
                "Command exec frame payload: {:?}",
                chirpstack_api::gw::GatewayCommandExecRequest::decode(pub_msg.payload.clone())
                    .expect("Debug")
            );
        }
        if pub_msg.topic.contains("command") && pub_msg.topic.contains("raw") {
            debug!(
                "Command raw frame payload: {:?}",
                chirpstack_api::gw::GatewayCommandExecRequest::decode(pub_msg.payload.clone())
                    .expect("Debug")
            );
        }
        if pub_msg.topic.contains("event") && pub_msg.topic.contains("stats") {
            debug!(
                "Event stats frame payload: {:?}",
                chirpstack_api::gw::GatewayStats::decode(pub_msg.payload.clone()).expect("Debug")
            );
        }
        if pub_msg.topic.contains("event") && pub_msg.topic.contains("up") {
            debug!(
                "Event up frame payload: {:?}",
                chirpstack_api::gw::UplinkFrame::decode(pub_msg.payload.clone()).expect("Debug")
            );
        }
        if pub_msg.topic.contains("event") && pub_msg.topic.contains("ack") {
            debug!(
                "Event ack frame payload: {:?}",
                chirpstack_api::gw::DownlinkTxAck::decode(pub_msg.payload.clone()).expect("Debug")
            );
        }
        if pub_msg.topic.contains("event") && pub_msg.topic.contains("exec") {
            debug!(
                "Event exec frame payload: {:?}",
                chirpstack_api::gw::GatewayCommandExecResponse::decode(pub_msg.payload.clone())
                    .expect("Debug")
            );
        }
        if pub_msg.topic.contains("event") && pub_msg.topic.contains("raw") {
            debug!(
                "Event raw frame payload: {:?}",
                chirpstack_api::gw::RawPacketForwarderEvent::decode(pub_msg.payload.clone())
                    .expect("Debug")
            );
        }
        if pub_msg.topic.contains("state") && pub_msg.topic.contains("conn") {
            debug!(
                "Event exec frame payload: {:?}",
                chirpstack_api::gw::ConnState::decode(pub_msg.payload.clone()).expect("Debug")
            );
        }
    }
}
