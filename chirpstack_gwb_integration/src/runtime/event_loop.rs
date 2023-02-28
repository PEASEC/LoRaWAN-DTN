//! The event loop processing incoming MQTT messages.

use crate::gateway_topics::ParsedTopic;
use crate::runtime::callbacks::{AllGatewaysCallbackStorage, PerGatewayCallbackStorage};
use prost::Message;
use rumqttc::{EventLoop, Incoming, Publish};
#[cfg(debug_assertions)]
use tracing::debug;
use tracing::{error, trace};

/// Runs the event loop processing incoming MQTT messages. Needs to be spawned in an async task and
/// kept running continuously.
#[tracing::instrument(skip_all)]
pub(crate) async fn run_event_loop(
    mut event_loop: EventLoop,
    per_gateway_callbacks: PerGatewayCallbackStorage,
    all_gateways_callbacks: AllGatewaysCallbackStorage,
) {
    loop {
        match event_loop.poll().await {
            Ok(notification) => {
                if let rumqttc::Event::Incoming(Incoming::Publish(pub_msg)) = notification {
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
                //TODO prevent flooding the log with error messages.

                error!(%e);
            }
        }
    }
}

/// Prints every published message received by the client. Only included in debug builds.
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
