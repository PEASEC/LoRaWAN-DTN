//! Types used for graceful shutdown.
//!
//! Agents are used in async tasks to check whether a shutdown should be initiated.
//! The controller receives all shutdown conditions and can initiate a shutdown.
//! The agents can be used to signal a shutdown condition to the shutdown controller.
//! The controller checks whether all agents have been dropped and if so finalizes the
//! shutdown. If not all agents have been dropped after a timeout, the shutdown is forced.
//!
//! Shutdown initiators are similar to agents but the controller will not wait for them to shut down.
//! This is useful for cases where a shutdown should be signaled but the component should not
//! be included in the graceful shutdown itself. The panic handler is one example.

use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time;
use tracing::{error, trace};

/// Possible conditions leading to a shutdown command.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ShutdownConditions {
    /// A panic occurred.
    Panic,
    /// A mqtt error occurred in the runtime event loop.
    MqttError,
    /// Retrieval of gateway IDs failed.
    GatewayRetrievalFailed,
    /// Axum server could not be started.
    AxumStartFailed,
    /// Spatz should be restarted.
    Restart,
}

/// Generator for shutdown agents and a shutdown controller.
#[derive(Debug)]
pub struct ShutdownGenerator {
    /// Receiver for a shutdown notification. When a message is received, the shutdown will
    /// be initialized by the agent.
    notify_rx: watch::Sender<()>,
    /// Transceiver for shutdown conditions. Used to send a shutdown condition to the
    /// shutdown controller.
    condition_tx: mpsc::Sender<ShutdownConditions>,
    /// Receiver for shutdown conditions. Used by the shutdown controller.
    condition_rx: mpsc::Receiver<ShutdownConditions>,
    /// Transceiver to indicate shutdown completion by shutdown agents.
    complete_indicator_tx: mpsc::Sender<()>,
    /// Receiver to check for shutdown completion by the shutdown controller.
    complete_indicator_rx: mpsc::Receiver<()>,
}

impl ShutdownGenerator {
    /// Creates a new [`ShutdownGenerator`].
    pub fn new() -> Self {
        let (notify_rx, _) = watch::channel(());
        let (condition_tx, condition_rx) = mpsc::channel(1);
        let (complete_indicator_tx, complete_indicator_rx) = mpsc::channel(1);

        Self {
            notify_rx,
            condition_tx,
            condition_rx,
            complete_indicator_tx,
            complete_indicator_rx,
        }
    }

    /// Generate a new [`ShutdownInitiator`]
    pub fn generate_initiator(&self) -> ShutdownInitiator {
        ShutdownInitiator::new(self.condition_tx.clone())
    }

    /// Generate a new [`ShutdownAgent`].
    pub fn generate_agent(&self) -> ShutdownAgent {
        ShutdownAgent::new(
            self.notify_rx.subscribe(),
            self.condition_tx.clone(),
            self.complete_indicator_tx.clone(),
        )
    }

    /// Consume the [`ShutdownGenerator`] and create a new [`ShutdownController`].
    pub fn generate_control(self) -> ShutdownController {
        ShutdownController::new(
            self.notify_rx,
            self.condition_rx,
            self.complete_indicator_rx,
        )
    }
}

/// Can send [`ShutdownConditions`] to the [`ShutdownController`] but is not waited for in the
/// shutdown process.
#[derive(Debug)]
pub struct ShutdownInitiator {
    /// Transceiver for [`ShutdownConditions`] to the [`ShutdownController`].
    condition_tx: mpsc::Sender<ShutdownConditions>,
}

impl ShutdownInitiator {
    /// Creates a new [`ShutdownInitiator`].
    fn new(condition_tx: mpsc::Sender<ShutdownConditions>) -> Self {
        Self { condition_tx }
    }

    /// Send a [`ShutdownConditions`] to the [`ShutdownController`].
    pub fn initiate_shutdown(&self, reason: ShutdownConditions) {
        trace!("Initiate shutdown: {reason:?}");
        if let Err(err) = self.condition_tx.try_send(reason) {
            error!(%err);
        }
    }
}

/// Manages shutdown conditions, notifies agents to shut down and provides a timeout for agents to
/// shut down.
#[derive(Debug)]
pub struct ShutdownController {
    /// Transceiver for shutdown notification.
    notify_tx: watch::Sender<()>,
    /// Receiver for shutdown conditions.
    condition_rx: mpsc::Receiver<ShutdownConditions>,
    /// Receiver to check for shutdown completion.
    complete_indicator_rx: mpsc::Receiver<()>,
}

impl ShutdownController {
    /// Creates a new [`ShutdownController`].
    fn new(
        notify_tx: watch::Sender<()>,
        condition_rx: mpsc::Receiver<ShutdownConditions>,
        complete_indicator_rx: mpsc::Receiver<()>,
    ) -> Self {
        Self {
            notify_tx,
            condition_rx,
            complete_indicator_rx,
        }
    }

    /// Starts the shutdown process.
    ///
    /// Sends a message via the `notify_tx` channel to all [`ShutdownAgent`].
    pub fn start_shutdown(&self) {
        trace!("Start shutdown");
        if self.notify_tx.receiver_count() > 0 {
            if let Err(err) = self.notify_tx.send(()) {
                error!(%err);
            }
        } else {
            trace!("No shutdown notify subscribers");
        }
    }

    /// Awaits all [`ShutdownAgent`] to shut down or until the timeout elapsed.
    ///
    /// Waits for all [`ShutdownAgent`] to drop their `complete_indicator_tx` transceiver.
    /// This causes the `complete_indicator_rx` to return with an error and signals no
    /// [`ShutdownAgent`] is still active.
    pub async fn await_complete_shutdown(&mut self, timeout_secs: u64) {
        tokio::select! {
            _ = time::sleep(Duration::from_secs(timeout_secs)) => {
                trace!("Timeout over, forcing shutdown");
            },
            _ = self.complete_indicator_rx.recv() => {}
        }
    }

    /// Awaits the [`ShutdownConditions`] receiver.
    pub async fn await_shutdown_initiation(&mut self) -> Option<ShutdownConditions> {
        self.condition_rx.recv().await
    }
}

/// Graceful shutdown mechanism.
///
/// Facilitates graceful shutdown of all tasks by listing on a broadcast channel for the
/// shutdown signal. Once received, shutdown should be initiated by the task. When the
/// [`ShutdownAgent`] struct is dropped, a [`mpsc::Sender<()>`] is also dropped which indicates
/// the shutdown is complete. When all [`ShutdownAgent`] structs have been dropped, the shutdown
/// can complete.
#[derive(Debug, Clone)]
pub struct ShutdownAgent {
    /// Whether a shutdown notification has been received.
    shutdown: bool,
    /// Receiver for a shutdown notification. When a message is received, the shutdown will
    /// be initialized by the agent.
    notify_rx: watch::Receiver<()>,
    /// Transceiver for shutdown conditions. Used to send a shutdown condition to the
    /// shutdown controller.
    condition_tx: mpsc::Sender<ShutdownConditions>,
    /// Transceiver to indicate shutdown completion. Must be dropped to signal completion.
    _complete_indicator_tx: mpsc::Sender<()>,
}

impl ShutdownAgent {
    /// Create a new [`ShutdownAgent`].
    fn new(
        notify_rx: watch::Receiver<()>,
        condition_tx: mpsc::Sender<ShutdownConditions>,
        complete_indicator_tx: mpsc::Sender<()>,
    ) -> Self {
        Self {
            shutdown: false,
            notify_rx,
            condition_tx,
            _complete_indicator_tx: complete_indicator_tx,
        }
    }

    /// Send a [`ShutdownConditions`] to the [`ShutdownController`].
    ///
    /// Also sets the `shutdown` value to true and will shut down the agent.
    pub fn initiate_shutdown(&mut self, reason: ShutdownConditions) {
        trace!("Initiate shutdown: {reason:?}");
        if let Err(err) = self.condition_tx.try_send(reason) {
            error!(%err);
        }
        self.shutdown = true;
    }

    /// Awaits a shutdown notification.
    pub async fn await_shutdown(&mut self) {
        if self.shutdown {
            return;
        }

        let _ = self.notify_rx.changed().await;

        self.shutdown = true;
    }
}
