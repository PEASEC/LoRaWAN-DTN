//! Collection and management of duty cycle information.

mod airtime_calculator;

use crate::error::{ConsumeDutyCycleTimeError, SubBandCreationError};
use crate::graceful_shutdown::ShutdownAgent;
use crate::AppState;
pub use airtime_calculator::calc_max_downlink_airtime;
use async_trait::async_trait;
use chirpstack_api::gw::DownlinkFrame;
use chirpstack_gwb_integration::runtime::callbacks::CommandDownCallback;
use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::log::trace;
use tracing::{error, instrument};

/// Downlink callback sends incoming downlink frames to the duty cycle collector task.
#[derive(Debug)]
pub struct DownlinkCallback {
    /// Channel to send the gateway ID and the downlink frame.
    pub downlink_callback_tx: mpsc::Sender<(String, DownlinkFrame)>,
}

#[async_trait]
impl CommandDownCallback for DownlinkCallback {
    /// Send observed downlink commands via the channel in the [`DownlinkCallback`] struct.
    async fn dispatch_down_command(&self, gateway_id: String, downlink_command: DownlinkFrame) {
        trace!("Dispatch down command called");
        if let Err(err) = self
            .downlink_callback_tx
            .try_send((gateway_id, downlink_command))
        {
            error!(%err);
        }
    }
}

#[instrument(skip_all)]
pub async fn downlink_duty_cycle_collector_task(
    mut downlink_rx: mpsc::Receiver<(String, DownlinkFrame)>,
    state: Arc<AppState>,
    mut shutdown_agent: ShutdownAgent,
) {
    trace!("Starting up");
    loop {
        let downlink = tokio::select! {
            downlink = downlink_rx.recv() => { downlink}
            _ = shutdown_agent.await_shutdown() => {
                trace!("Shutting down");
                return
            }
        };

        if let Some((gateway_id, downlink)) = downlink {
            trace!("Received downlink for gateway \"{gateway_id}\"");
            let (freq, airtime) = match calc_max_downlink_airtime(downlink) {
                Ok(airtime) => airtime,
                Err(err) => {
                    error!(%err);
                    continue;
                }
            };
            trace!("Max airtime for downlink on frequency {freq}: {airtime}");

            {
                if let Err(err) = state
                    .duty_cycle_manager
                    .lock()
                    .await
                    .consume_capacity(airtime, freq, gateway_id)
                {
                    error!(%err);
                }
            }
        }
    }
}

/// Sub band of the EU 868MHz to 870MHz band.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum EuSubBand {
    Sb863000_865000,
    Sb865000_868000,
    Sb868000_868600,
    Sb868700_869200,
    Sb869400_869650,
    Sb869700_870000,
}

impl EuSubBand {
    /// Duty cycle limitations according to "ETSI EN 300 220-2 V3.2.1 (2018-06)" page 21.
    /// <https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.01_60/en_30022002v030201p.pdf>
    #[allow(clippy::match_same_arms)]
    pub fn duty_cycle(self) -> f64 {
        match self {
            EuSubBand::Sb863000_865000 => 0.001,
            EuSubBand::Sb865000_868000 => 0.01,
            EuSubBand::Sb868000_868600 => 0.01,
            EuSubBand::Sb868700_869200 => 0.001,
            EuSubBand::Sb869400_869650 => 0.1,
            EuSubBand::Sb869700_870000 => 0.01,
        }
    }

    /// Tries to create a [`EuSubBand`] from the frequency in Hz.
    ///
    /// # Errors
    ///
    /// Returns an error if the provided frequency does not match any sub band.
    pub fn try_from_freq(freq: u32) -> Result<Self, SubBandCreationError> {
        match freq {
            863_000_000..=865_000_000 => Ok(EuSubBand::Sb863000_865000),
            865_000_001..=868_000_000 => Ok(EuSubBand::Sb865000_868000),
            868_000_001..=868_600_000 => Ok(EuSubBand::Sb868000_868600),
            868_700_000..=869_200_000 => Ok(EuSubBand::Sb868700_869200),
            869_400_000..=869_650_000 => Ok(EuSubBand::Sb869400_869650),
            869_700_000..=870_000_000 => Ok(EuSubBand::Sb869700_870000),
            freq => Err(SubBandCreationError::NoMatchingSubBand { freq }),
        }
    }
}

/// Collects and manages duty cycle information for all gateways.
///
/// Keeps track of the amount of time already used for every sub band for every gateway.
#[derive(Debug)]
pub struct DutyCycleManager {
    /// Data storage for every sub band.
    gateways: HashMap<String, PerGatewayDutyCycleManager>,
}

impl DutyCycleManager {
    /// Creates a new [`DutyCycleManager`].
    pub fn new(gateways: HashMap<String, PerGatewayDutyCycleManager>) -> Self {
        Self { gateways }
    }

    /// Returns the current duty cycle information per gateway.
    pub fn stats(&self) -> HashMap<String, PerGatewayDutyCycleManager> {
        self.gateways.clone()
    }

    /// Returns whether the needed capacity is still available for the gateway in the sub band of the provided frequency.
    ///
    /// Adds a new entry for gateways not yet in the duty cycle manager.
    /// # Errors
    ///
    /// Returns an error if the frequency does not match any sub band.
    pub fn is_capacity_available(
        &mut self,
        needed_capacity: f64,
        freq: u32,
        gateway_id: String,
    ) -> Result<bool, SubBandCreationError> {
        match self.gateways.entry(gateway_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().is_capacity_available(needed_capacity, freq)
            }
            Entry::Vacant(entry) => {
                let entry = entry.insert(PerGatewayDutyCycleManager::new());
                entry.is_capacity_available(needed_capacity, freq)
            }
        }
    }

    /// Consumes the provided capacity for the gateway in the sub band corresponding to the provided frequency.
    ///
    /// Adds a new entry for gateways not yet in the duty cycle manager.
    /// # Errors
    ///
    /// Returns an error if:
    /// - the frequency does not match any sub band.
    /// - there was not capacity left in the sub band.
    pub fn consume_capacity(
        &mut self,
        used_capacity: f64,
        freq: u32,
        gateway_id: String,
    ) -> Result<(), ConsumeDutyCycleTimeError> {
        trace!("Consume capacity for gateway: {gateway_id}");
        match self.gateways.entry(gateway_id) {
            Entry::Occupied(mut entry) => entry.get_mut().consume_capacity(used_capacity, freq),
            Entry::Vacant(entry) => {
                let entry = entry.insert(PerGatewayDutyCycleManager::new());
                entry.consume_capacity(used_capacity, freq)
            }
        }
    }
}

/// Collects and manages duty cycle information for one gateway.
///
/// Keeps track of the amount of time already used for every sub band.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerGatewayDutyCycleManager {
    /// Data storage for every sub band.
    bands: HashMap<EuSubBand, Vec<(chrono::DateTime<Utc>, f64)>>,
}

impl Default for PerGatewayDutyCycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PerGatewayDutyCycleManager {
    /// Creates a new [`PerGatewayDutyCycleManager`].
    pub fn new() -> Self {
        let mut bands = HashMap::new();
        bands.insert(EuSubBand::Sb863000_865000, Vec::new());
        bands.insert(EuSubBand::Sb865000_868000, Vec::new());
        bands.insert(EuSubBand::Sb868000_868600, Vec::new());
        bands.insert(EuSubBand::Sb868700_869200, Vec::new());
        bands.insert(EuSubBand::Sb869400_869650, Vec::new());
        bands.insert(EuSubBand::Sb869700_870000, Vec::new());
        Self { bands }
    }

    /// Removes all entries of the capacity vec older than one hour.
    fn remove_outdated_capacity(&mut self) {
        let now = Utc::now();
        for capacity_vec in self.bands.values_mut() {
            let mut i = 0;
            while i < capacity_vec.len() {
                if (now - capacity_vec[i].0).num_minutes() > 60 {
                    let _ = capacity_vec.remove(i);
                } else {
                    i += 1;
                }
            }
        }
    }

    /// Calculates the capacity currently used for the provided band.
    fn calculate_used_capacity(&mut self, band: EuSubBand) -> f64 {
        self.remove_outdated_capacity();
        let used_capacity = self
            .bands
            .get(&band)
            .expect("Band is missing, should be added in new()");
        used_capacity
            .iter()
            .fold(0.0, |sum, (_, capacity)| sum + capacity)
    }

    /// Returns whether the needed capacity is still available in the sub band of the provided frequency.
    ///
    /// # Errors
    ///
    /// Returns an error if the frequency does not match any sub band.
    pub fn is_capacity_available(
        &mut self,
        needed_capacity: f64,
        freq: u32,
    ) -> Result<bool, SubBandCreationError> {
        let band = EuSubBand::try_from_freq(freq)?;
        // 3600000.0ms in one hour
        let max_capacity = band.duty_cycle() * 3_600_000.0;

        Ok(max_capacity >= self.calculate_used_capacity(band) + needed_capacity)
    }

    /// Consumes the provided capacity in the sub band corresponding to the provided frequency.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the frequency does not match any sub band.
    /// - there was not capacity left in the sub band.
    pub fn consume_capacity(
        &mut self,
        used_capacity: f64,
        freq: u32,
    ) -> Result<(), ConsumeDutyCycleTimeError> {
        if self.is_capacity_available(used_capacity, freq)? {
            let band = EuSubBand::try_from_freq(freq)?;
            let capacity_vec = self
                .bands
                .get_mut(&band)
                .expect("Band is missing, should be added in new()");
            capacity_vec.push((Utc::now(), used_capacity));

            if cfg!(debug_assertions) {
                let capacity = self.calculate_used_capacity(band);
                trace!(
                    "Used {capacity} of {} in band {band:?}",
                    band.duty_cycle() * 3_600_000.0,
                );
            }

            Ok(())
        } else {
            Err(ConsumeDutyCycleTimeError::CapacityOverused)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::duty_cycle_manager::{EuSubBand, PerGatewayDutyCycleManager};
    use crate::error::ConsumeDutyCycleTimeError;
    use chrono::{Duration, Utc};

    #[allow(clippy::unwrap_used)]
    #[test]
    fn remove_outdated_capacity() {
        let mut pg_duty_cycle_manager = PerGatewayDutyCycleManager::new();
        let band = pg_duty_cycle_manager
            .bands
            .get_mut(&EuSubBand::Sb863000_865000)
            .unwrap();
        band.push((Utc::now() - Duration::minutes(65), 100.0));
        assert!(!band.is_empty());
        pg_duty_cycle_manager.remove_outdated_capacity();
        let band = pg_duty_cycle_manager
            .bands
            .get_mut(&EuSubBand::Sb863000_865000)
            .unwrap();
        assert!(band.is_empty());
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn consume_capacity() {
        let mut pg_duty_cycle_manager = PerGatewayDutyCycleManager::new();
        let band = pg_duty_cycle_manager
            .bands
            .get_mut(&EuSubBand::Sb863000_865000)
            .unwrap();
        band.push((Utc::now() - Duration::minutes(65), f64::MAX));
        assert_eq!(
            Ok(()),
            pg_duty_cycle_manager.consume_capacity(
                EuSubBand::Sb863000_865000.duty_cycle() * 3_600_000.0,
                863_000_000
            )
        );
        assert_eq!(
            Err(ConsumeDutyCycleTimeError::CapacityOverused),
            pg_duty_cycle_manager.consume_capacity(1.0, 863_000_000)
        );
    }
}
