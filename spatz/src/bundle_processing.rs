use crate::send_buffers::BundleSendBuffer;
use chirpstack_gwb_integration::downlinks::predefined_parameters::DataRate;
use tracing::{error, instrument, trace};

/// Async task to process incoming bundle from the `bundles_from_ws_receiver` channel.
/// Creates a [`BundleSendBuffer`] from the incoming [`bp7::Bundle`].
#[instrument(skip(bundles_from_ws_receiver))]
pub async fn bundles_processor_task(
    mut bundles_from_ws_receiver: tokio::sync::mpsc::Receiver<bp7::Bundle>,
    bundle_send_buffer_sender: tokio::sync::mpsc::Sender<BundleSendBuffer>,
) {
    while let Some(bundle) = bundles_from_ws_receiver.recv().await {
        trace!("Received bundle: {bundle}");

        //TODO Add dynamic decision making for data rate

        match BundleSendBuffer::from_bp7_bundle(bundle, DataRate::Eu863_870Dr0) {
            Ok(send_buffer) => {
                if let Err(err) = bundle_send_buffer_sender.try_send(send_buffer) {
                    error!(%err);
                }
            }
            Err(err) => {
                error!(%err);
            }
        }
    }
    trace!("Leaving bundles processor task.");
}
