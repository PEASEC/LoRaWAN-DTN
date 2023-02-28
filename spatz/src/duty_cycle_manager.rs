mod airtime_calculator;

use crate::error::{DutyCycleManagerError, SubBandError};
use crate::AppState;
pub use airtime_calculator::calc_max_downlink_airtime;
use async_trait::async_trait;
use chirpstack_api::gw::DownlinkFrame;
use chirpstack_gwb_integration::runtime::callbacks::CommandDownCallback;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::log::trace;
use tracing::{error, instrument};

/// Uplink callback sending incoming uplink frames to the uplink processing task.
#[derive(Debug)]
pub struct DownlinkCallback {
    pub sender: tokio::sync::mpsc::Sender<(String, DownlinkFrame)>,
}

#[async_trait]
impl CommandDownCallback for DownlinkCallback {
    /// Send observed downlink commands via the channel in the [`DownlinkCallback`] struct.
    async fn dispatch_down_command(&self, gateway_id: String, downlink_command: DownlinkFrame) {
        trace!("Dispatch down command called");
        if let Err(err) = self.sender.try_send((gateway_id, downlink_command)) {
            error!(%err);
        }
    }
}

#[instrument(skip_all)]
pub async fn downlink_duty_cycle_collector_task(
    mut downlink_receiver: tokio::sync::mpsc::Receiver<(String, DownlinkFrame)>,
    state: Arc<AppState>,
) {
    while let Some((gateway_id, downlink)) = downlink_receiver.recv().await {
        trace!("Received downlink from gateway \"{gateway_id}\"");
        let (freq, airtime) = match calc_max_downlink_airtime(downlink) {
            Ok(airtime) => airtime,
            Err(err) => {
                error!(%err);
                continue;
            }
        };
        trace!("Max airtime for downlink on frequency {freq}: {airtime}");

        {
            let mut duty_cycle_manager_lock =
                state.duty_cycle_manager.lock().expect("Lock poisoned");
            if let Err(err) = duty_cycle_manager_lock.use_capacity(airtime, freq) {
                error!(%err);
            }
        }
    }
}

/// Task to execute [`DutyCycleManager::reset()`] every hour.
#[instrument(skip_all)]
pub async fn duty_cycle_reset_task(state: Arc<AppState>) {
    loop {
        {
            state
                .duty_cycle_manager
                .lock()
                .expect("Lock poisoned")
                .reset();
        }
        trace!("Duty cycle manager reset");
        tokio::time::sleep(tokio::time::Duration::from_secs(60 * 60)).await;
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
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
    /// https://www.etsi.org/deliver/etsi_en/300200_300299/30022002/03.02.01_60/en_30022002v030201p.pdf
    pub fn duty_cycle(&self) -> f64 {
        match self {
            EuSubBand::Sb863000_865000 => 0.001,
            EuSubBand::Sb865000_868000 => 0.01,
            EuSubBand::Sb868000_868600 => 0.01,
            EuSubBand::Sb868700_869200 => 0.001,
            EuSubBand::Sb869400_869650 => 0.1,
            EuSubBand::Sb869700_870000 => 0.01,
        }
    }

    pub fn try_from_freq(freq: u32) -> Result<Self, SubBandError> {
        match freq {
            863000000..=865000000 => Ok(EuSubBand::Sb863000_865000),
            865000001..=868000000 => Ok(EuSubBand::Sb865000_868000),
            868000001..=868600000 => Ok(EuSubBand::Sb868000_868600),
            868700000..=869200000 => Ok(EuSubBand::Sb868700_869200),
            869400000..=869650000 => Ok(EuSubBand::Sb869400_869650),
            869700000..=870000000 => Ok(EuSubBand::Sb869700_870000),
            freq => Err(SubBandError::NoMatchingSubBand { freq }),
        }
    }
}

#[derive(Debug)]
pub struct DutyCycleManager {
    bands: HashMap<EuSubBand, f64>,
}

impl Default for DutyCycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DutyCycleManager {
    pub fn new() -> Self {
        let mut bands = HashMap::new();
        bands.insert(EuSubBand::Sb863000_865000, 0.0);
        bands.insert(EuSubBand::Sb865000_868000, 0.0);
        bands.insert(EuSubBand::Sb868000_868600, 0.0);
        bands.insert(EuSubBand::Sb868700_869200, 0.0);
        bands.insert(EuSubBand::Sb869400_869650, 0.0);
        bands.insert(EuSubBand::Sb869700_870000, 0.0);
        Self { bands }
    }

    pub fn reset(&mut self) {
        self.bands.iter_mut().for_each(|(_, value)| *value = 0.0);
    }

    pub fn capacity_available(
        &self,
        needed_capacity: f64,
        freq: u32,
    ) -> Result<bool, DutyCycleManagerError> {
        let band = EuSubBand::try_from_freq(freq)?;
        // 3600000.0ms in one hour
        let max_capacity = band.duty_cycle() * 3600000.0;
        let used_capacity = self
            .bands
            .get(&band)
            .expect("Band is missing, should be added in new()");
        Ok(max_capacity >= used_capacity + needed_capacity)
    }

    pub fn use_capacity(
        &mut self,
        used_capacity: f64,
        freq: u32,
    ) -> Result<(), DutyCycleManagerError> {
        if self.capacity_available(used_capacity, freq)? {
            let band = EuSubBand::try_from_freq(freq)?;
            let capacity = self
                .bands
                .get_mut(&band)
                .expect("Band is missing, should be added in new()");
            *capacity += used_capacity;
            trace!(
                "Used {capacity} of {} in band {band:?}",
                band.duty_cycle() * 3600000.0
            );

            Ok(())
        } else {
            Err(DutyCycleManagerError::CapacityOverused)
        }
    }
}
