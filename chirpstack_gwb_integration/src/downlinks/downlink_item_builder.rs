//! Builders for downlink items.

use crate::downlinks::predefined_parameters::{
    Bandwidth, CodingRate, DataRate, Frequency, SpreadingFactor,
};
use crate::downlinks::{
    DelayTimingClassA, DelayTimingInfo, DownlinkItem, GpsEpochTimingInfo, GpsTimingClassB,
    ImmediatelyClassC, LoRaModulationInfo, TxInfo,
};
use crate::error::DownlinkError;
use std::marker::PhantomData;

/// Populate with data and build [`DownlinkItem`].
#[derive(Debug, Clone)]
pub struct DownlinkItemBuilder<DownlinkType> {
    phy_payload: Option<Vec<u8>>,
    frequency: Option<u32>,
    power: Option<i32>,
    data_rate: Option<DataRate>,
    bandwidth: Option<u32>,
    spreading_factor: Option<u32>,
    code_rate: Option<chirpstack_api::gw::CodeRate>,
    polarization_inversion: Option<bool>,
    board: Option<u32>,
    antenna: Option<u32>,
    delay: Option<std::time::Duration>,
    context: Option<Vec<u8>>,
    time_since_gps_epoch: Option<std::time::Duration>,
    downlink_type: PhantomData<DownlinkType>,
}

impl<DownlinkType> DownlinkItemBuilder<DownlinkType> {
    /// Set payload.
    #[must_use]
    pub fn phy_payload(mut self, payload: Vec<u8>) -> DownlinkItemBuilder<DownlinkType> {
        self.phy_payload = Some(payload);
        self
    }

    /// Set frequency. Use [`frequency()`](ItemBuilder::frequency()) with [`Frequency`] for predefined options.
    pub fn frequency_raw(mut self, frequency: u32) -> DownlinkItemBuilder<DownlinkType> {
        self.frequency = Some(frequency);
        self
    }

    /// Set frequency.
    #[must_use]
    pub fn frequency(self, frequency: Frequency) -> DownlinkItemBuilder<DownlinkType> {
        match frequency {
            Frequency::Freq868_1 => self.frequency_raw(868100000),
            Frequency::Freq868_3 => self.frequency_raw(868300000),
            Frequency::Freq868_5 => self.frequency_raw(868500000),
        }
    }

    /// Set power.
    #[must_use]
    pub fn power(mut self, power: i32) -> DownlinkItemBuilder<DownlinkType> {
        self.power = Some(power);
        self
    }

    /// Set data rate.
    /// Using `data_rate()` instead of setting bandwidth and spreading factor enables payload size
    /// checking.
    #[must_use]
    pub fn data_rate(mut self, data_rate: DataRate) -> DownlinkItemBuilder<DownlinkType> {
        let (bandwidth, spreading_factor) = data_rate.into_bandwidth_and_spreading_factor();
        self.bandwidth = Some(bandwidth.hz());
        self.spreading_factor = Some(spreading_factor as u32);
        self.data_rate = Some(data_rate);
        self
    }

    /// Set bandwidth. Use [`data_rate()`](ItemBuilder::data_rate()) with [`DataRate`] for predefined options.
    pub fn raw_bandwidth(&mut self, bandwidth: Bandwidth) -> &mut Self {
        let bandwidth = match bandwidth {
            Bandwidth::Bw125 => 125000,
            Bandwidth::Bw250 => 250000,
        };
        self.bandwidth = Some(bandwidth);
        self
    }

    /// Set spreading factor. Use [`data_rate()`](ItemBuilder::data_rate()) with [`DataRate`] for predefined options.
    pub fn raw_spreading_factor(&mut self, spreading_factor: SpreadingFactor) -> &mut Self {
        self.spreading_factor = Some(spreading_factor as u32);
        self
    }

    /// Set code rate. Defaults to `4/5`.
    pub fn code_rate(&mut self, code_rate: CodingRate) -> &mut Self {
        let code_rate = match code_rate {
            CodingRate::Cr45 => chirpstack_api::gw::CodeRate::Cr45,
        };
        self.code_rate = Some(code_rate);
        self
    }

    /// Set polarization inversion. Defaults to `false`.
    pub fn polarization_inversion(&mut self, polarization_inversion: bool) -> &mut Self {
        self.polarization_inversion = Some(polarization_inversion);
        self
    }

    /// Set board.
    #[must_use]
    pub fn board(&mut self, board: u32) -> &mut Self {
        self.board = Some(board);
        self
    }

    /// Set antenna.
    #[must_use]
    pub fn antenna(&mut self, antenna: u32) -> &mut Self {
        self.antenna = Some(antenna);
        self
    }

    /// Set the downlink context.
    pub fn context(&mut self, context: Vec<u8>) -> &mut Self {
        self.context = Some(context);
        self
    }

    /// Build a [`DownlinkItem`] with base parameters (shared by all variants).
    fn build_base(&mut self) -> Result<DownlinkItem<DownlinkType>, DownlinkError> {
        // redundant for now, might prevent issues if more types get added
        self.check_base_for_plausibility()?;
        if self.context.is_none() {
            self.context = Some(Vec::new());
        }
        Ok(DownlinkItem {
            phy_payload: self
                .phy_payload
                .clone()
                .expect("This can't happen, phy_payload is checked for None before."),
            tx_info: TxInfo {
                frequency: self
                    .frequency
                    .expect("This can't happen, frequency is checked for None before."),
                power: self
                    .power
                    .expect("This can't happen, power is checked for None before."),
                lo_ra_modulation_info: LoRaModulationInfo {
                    bandwidth: self.bandwidth.expect(
                        "This can't happen, lo_ra_modulation_info is checked for None before.",
                    ),
                    spreading_factor: self
                        .spreading_factor
                        .expect("This can't happen, spreading_factor is checked for None before."),
                    code_rate: self.code_rate.expect(
                        "This can't happen, lo_ra_modulation_info is checked for None before.",
                    ),
                    polarization_inversion: self.polarization_inversion.expect(
                        "This can't happen, polarization_inversion is checked for None before.",
                    ),
                },
                board: self
                    .board
                    .expect("This can't happen, board is checked for None before."),
                antenna: self
                    .antenna
                    .expect("This can't happen, antenna is checked for None before."),
                delay_timing_info: None,
                context: self.context.clone(),
                gps_epoch_timing_info: None,
                downlink_type: PhantomData::<DownlinkType>,
            },
        })
    }

    /// Check whether the set parameters are plausible.
    /// Payload size checking is only available if [`data_rate`](DownlinkItemBuilder.data_rate) is set.
    fn check_base_for_plausibility(&self) -> Result<(), DownlinkError> {
        if self.phy_payload.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "phy_payload".to_owned(),
            });
        }
        if self.frequency.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "frequency".to_owned(),
            });
        }
        if self.power.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "power".to_owned(),
            });
        }
        if self.bandwidth.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "bandwidth".to_owned(),
            });
        }
        if self.spreading_factor.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "spreading_factor".to_owned(),
            });
        }
        if self.code_rate.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "code_rate".to_owned(),
            });
        }
        if self.polarization_inversion.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "polarization_inverse".to_owned(),
            });
        }
        if self.board.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "board".to_owned(),
            });
        }
        if self.antenna.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "antenna".to_owned(),
            });
        }

        // Payload size checking is only enabled if `self.data_rate` is set.
        if self.data_rate.is_some() {
            self.data_rate
                .as_ref()
                .expect("This can't happen, data_rate is checked for None before.")
                .check_payload_size(
                    self.phy_payload
                        .as_ref()
                        .expect("This can't happen, phy_payload is checked for None before.")
                        .len(),
                )?
        }

        Ok(())
    }
}

impl Default for DownlinkItemBuilder<DelayTimingClassA> {
    fn default() -> Self {
        DownlinkItemBuilder {
            phy_payload: None,
            frequency: None,
            power: None,
            data_rate: None,
            bandwidth: None,
            spreading_factor: None,
            code_rate: Some(chirpstack_api::gw::CodeRate::Cr45),
            polarization_inversion: Some(false),
            board: None,
            antenna: None,
            delay: None,
            context: None,
            time_since_gps_epoch: None,
            downlink_type: PhantomData::<DelayTimingClassA>,
        }
    }
}

impl DownlinkItemBuilder<DelayTimingClassA> {
    /// Create a new [`DownlinkItemBuilder<DelayTimingClassA>`].
    pub fn new() -> Self {
        DownlinkItemBuilder::default()
    }

    /// Set downlink delay.
    pub fn delay(&mut self, delay: std::time::Duration) -> &mut Self {
        self.delay = Some(delay);
        self
    }

    /// Check whether the set parameters are plausible.
    pub fn check_for_plausibility(&self) -> Result<(), DownlinkError> {
        self.check_base_for_plausibility()?;
        if self.delay.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "delay".to_owned(),
            });
        }

        Ok(())
    }

    /// Build the [DownlinkItem].
    pub fn build(&mut self) -> Result<DownlinkItem<DelayTimingClassA>, DownlinkError> {
        self.check_for_plausibility()?;
        let mut item = self.build_base()?;
        item.tx_info.delay_timing_info = Some(DelayTimingInfo {
            delay: self
                .delay
                .expect("This can't happen, variable is checked for None before."),
        });
        item.tx_info.context = self.context.clone();
        Ok(item)
    }
}

impl Default for DownlinkItemBuilder<GpsTimingClassB> {
    fn default() -> Self {
        DownlinkItemBuilder {
            phy_payload: None,
            frequency: None,
            power: None,
            data_rate: None,
            bandwidth: None,
            spreading_factor: None,
            code_rate: Some(chirpstack_api::gw::CodeRate::Cr45),
            polarization_inversion: Some(false),
            board: None,
            antenna: None,
            delay: None,
            context: None,
            time_since_gps_epoch: None,
            downlink_type: PhantomData::<GpsTimingClassB>,
        }
    }
}

impl DownlinkItemBuilder<GpsTimingClassB> {
    /// Create a new [`DownlinkItemBuilder<GpsTimingClassB>`].
    pub fn new() -> Self {
        DownlinkItemBuilder::default()
    }

    /// Set time since GPS epoch.
    pub fn time_since_gps_epoch(&mut self, time_since_gps_epoch: std::time::Duration) -> &mut Self {
        self.time_since_gps_epoch = Some(time_since_gps_epoch);
        self
    }

    /// Check whether the set parameters are plausible.
    pub fn check_for_plausibility(&self) -> Result<(), DownlinkError> {
        self.check_base_for_plausibility()?;
        if self.time_since_gps_epoch.is_none() {
            return Err(DownlinkError::MissingParameter {
                missing: "time_since_gps_epoch".to_owned(),
            });
        }
        Ok(())
    }

    /// Build the [`DownlinkItem`].
    pub fn build(&mut self) -> Result<DownlinkItem<GpsTimingClassB>, DownlinkError> {
        self.check_for_plausibility()?;
        let mut item = self.build_base()?;
        item.tx_info.gps_epoch_timing_info = Some(GpsEpochTimingInfo {
            time_since_gps_epoch: self
                .time_since_gps_epoch
                .expect("This can't happen, variable is checked for None before."),
        });
        Ok(item)
    }
}

impl Default for DownlinkItemBuilder<ImmediatelyClassC> {
    fn default() -> Self {
        DownlinkItemBuilder {
            phy_payload: None,
            frequency: None,
            power: None,
            data_rate: None,
            bandwidth: None,
            spreading_factor: None,
            code_rate: Some(chirpstack_api::gw::CodeRate::Cr45),
            polarization_inversion: Some(false),
            board: None,
            antenna: None,
            delay: None,
            context: None,
            time_since_gps_epoch: None,
            downlink_type: PhantomData::<ImmediatelyClassC>,
        }
    }
}

impl DownlinkItemBuilder<ImmediatelyClassC> {
    /// Create a new [`DownlinkItemBuilder<ImmediatelyClassC>`].
    pub fn new() -> Self {
        DownlinkItemBuilder::default()
    }

    /// Check whether the set parameters are plausible.
    pub fn check_for_plausibility(&self) -> Result<(), DownlinkError> {
        self.check_base_for_plausibility()?;
        Ok(())
    }

    /// Build the [`DownlinkItem`].
    pub fn build(&mut self) -> Result<DownlinkItem<ImmediatelyClassC>, DownlinkError> {
        self.check_for_plausibility()?;
        self.build_base()
    }
}
