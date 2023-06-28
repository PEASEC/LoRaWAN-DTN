//! Processing of incoming bundles.

use crate::graceful_shutdown::ShutdownAgent;
use crate::send_buffers::BundleSendBuffer;
use tokio::sync::mpsc;
use tracing::{error, instrument, trace};

/// Async task to process incoming bundle from the `bundles_from_ws_receiver` channel.
/// Creates a [`BundleSendBuffer`] from the incoming [`bp7::Bundle`].
#[instrument(skip_all)]
pub async fn bundles_processor_task(
    mut bundles_from_ws_rx: mpsc::Receiver<bp7::Bundle>,
    bundle_send_buffer_tx: mpsc::Sender<BundleSendBuffer>,
    mut shutdown_agent: ShutdownAgent,
) {
    trace!("Starting up");
    loop {
        let bundle = tokio::select! {
            bundle = bundles_from_ws_rx.recv() => { bundle}
            _ = shutdown_agent.await_shutdown() => {
                trace!("Shutting down");
                return
            }
        };
        if let Some(bundle) = bundle {
            trace!("Received bundle: {bundle}");

            match BundleSendBuffer::try_from(bundle) {
                Ok(send_buffer) => {
                    if let Err(err) = bundle_send_buffer_tx.try_send(send_buffer) {
                        error!(%err);
                    }
                }
                Err(err) => {
                    error!(%err);
                }
            }
        }
    }
}
