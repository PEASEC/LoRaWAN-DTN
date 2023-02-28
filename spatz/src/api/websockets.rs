use crate::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tracing::{error, trace};

/// On successful upgrade, hands connections off to the [`handle_socket`] function.
pub async fn ws_handler(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handles websocket connections. Incoming bundles are sent via channel to be processed.
/// Via LoRaWAN received bundles are sent as CBOR and JSON encoded binary and strict respectively.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let mut bundles_to_ws_receiver = state.bundles_to_ws.subscribe();
    let bundles_from_ws_sender = state.bundles_from_ws.clone();

    trace!("Spawning WS receiver task.");
    tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            if let Ok(msg) = msg {
                match msg {
                    Message::Text(t) => {
                        trace!("Received text message: {}", t);
                        match serde_json::from_str::<bp7::Bundle>(&t) {
                            Ok(bundle) => {
                                trace!("received bundle via text message: {:?}", bundle);
                                if let Err(err) = bundles_from_ws_sender.try_send(bundle) {
                                    error!(%err);
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Could not deserialize bundle received via text message: {e:?}"
                                )
                            }
                        }
                    }
                    Message::Binary(payload) => {
                        match serde_cbor::from_slice::<bp7::Bundle>(&payload) {
                            Ok(bundle) => {
                                trace!("received bundle via binary message: {:?}", bundle);
                                if let Err(err) = bundles_from_ws_sender.try_send(bundle) {
                                    error!(%err);
                                }
                            }
                            Err(e) => {
                                error!("Could not deserialize bundle received via binary message: {e:?}")
                            }
                        }
                    }
                    Message::Ping(_) => {
                        trace!("socket ping");
                    }
                    Message::Pong(_) => {
                        trace!("socket pong");
                    }
                    Message::Close(_) => {
                        trace!("client disconnected");
                        return;
                    }
                }
            } else {
                trace!("client disconnected");
                return;
            }
        }
    });

    trace!("Spawning WS sender task.");
    tokio::spawn(async move {
        while let Ok(mut bundle) = bundles_to_ws_receiver.recv().await {
            trace!("Sending bundle via WS as CBOR binary.");
            if let Err(err) = ws_sender.send(Message::Binary(bundle.to_cbor())).await {
                error!(%err);
            };
            trace!("Sending bundle via WS as JSON text.");
            if let Err(err) = ws_sender.send(Message::Text(bundle.to_json())).await {
                error!(%err);
            };
        }
    });
}
