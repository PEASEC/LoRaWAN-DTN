//! Builders for downlink items.

use crate::downlinks::predefined_parameters::{
    Bandwidth, CodingRate, DataRate, Frequency, SpreadingFactor,
};
use crate::downlinks::{
    DelayTimingClassA, DelayTimingInfo, DownlinkItem, DownlinkType, GpsEpochTimingInfo,
    GpsTimingClassB, ImmediatelyClassC, LoRaModulationInfo, TxInfo,
};
use crate::error::DownlinkItemBuilderError;
use std::hash::Hash;
use std::marker::PhantomData;

/// Builder for [`DownlinkItem`].
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct DownlinkItemBuilder<Dt>
where
    Dt: DownlinkType + Eq + PartialEq + Hash,
{
    /// Physical payload
    phy_payload: Option<Vec<u8>>,
    /// Frequency in Hz
    frequency: Option<u32>,
    /// Power in dBm.
    power: Option<i32>,
    /// Data rate.
    data_rate: Option<DataRate>,
    /// Bandwidth.
    bandwidth: Option<u32>,
    /// Spreading Factor.
    spreading_factor: Option<u32>,
    /// Code rate.
    code_rate: Option<chirpstack_api::gw::CodeRate>,
    /// Polarization inversion, true for downlinks, false for uplinks.
    ///
    /// Set to false if gateways should receive the downlink.
    polarization_inversion: Option<bool>,
    /// The board identifier for emitting the frame.
    ///
    /// (From <https://docs.rs/chirpstack_api/4.1.1/chirpstack_api/gw/struct.DownlinkTxInfo.html>)
    board: Option<u32>,
    /// The antenna identifier for emitting the frame.
    ///
    /// (From <https://docs.rs/chirpstack_api/4.1.1/chirpstack_api/gw/struct.DownlinkTxInfo.html>)
    antenna: Option<u32>,
    /// Delay (duration). The delay will be added to the gateway internal timing, provided by the context object.
    ///
    /// (From <https://docs.rs/chirpstack_api/4.1.1/chirpstack_api/gw/struct.DelayTimingInfo.html>)
    delay: Option<std::time::Duration>,
    /// Gateway specific context. In case of a Class-A downlink, this contains a copy of the uplink context.
    ///
    /// (From <https://docs.rs/chirpstack_api/4.1.1/chirpstack_api/gw/struct.DownlinkTxInfo.html>)
    context: Option<Vec<u8>>,
    /// Duration since GPS Epoch.
    ///
    /// (From <https://docs.rs/chirpstack_api/4.1.1/chirpstack_api/gw/struct.GpsEpochTimingInfo.html>)
    time_since_gps_epoch: Option<std::time::Duration>,
    /// The type of downlink, see [`DownlinkType`].
    downlink_type: PhantomData<Dt>,
}

impl<Dt> DownlinkItemBuilder<Dt>
where
    Dt: DownlinkType + Eq + PartialEq + Hash,
{
    /// Sets payload.
    pub fn phy_payload(&mut self, payload: Vec<u8>) -> &mut Self {
        self.phy_payload = Some(payload);
        self
    }

    /// Sets frequency.
    ///
    /// Use [`frequency()`](ItemBuilder::frequency()) with [`Frequency`] for predefined options.
    pub fn frequency_raw(&mut self, frequency: u32) -> &mut Self {
        self.frequency = Some(frequency);
        self
    }

    /// Sets frequency.
    pub fn frequency(&mut self, frequency: Frequency) -> &mut Self {
        match frequency {
            Frequency::Freq868_1 => self.frequency_raw(868_100_000),
            Frequency::Freq868_3 => self.frequency_raw(868_300_000),
            Frequency::Freq868_5 => self.frequency_raw(868_500_000),
        }
    }

    /// Sets power.
    pub fn power(&mut self, power: i32) -> &mut Self {
        self.power = Some(power);
        self
    }

    /// Sets data rate.
    ///
    /// Using `data_rate()` instead of setting bandwidth and spreading factor enables payload size
    /// checking.
    pub fn data_rate(&mut self, data_rate: DataRate) -> &mut Self {
        let (bandwidth, spreading_factor) = data_rate.into_bandwidth_and_spreading_factor();
        self.bandwidth = Some(bandwidth.hz());
        self.spreading_factor = Some(spreading_factor as u32);
        self.data_rate = Some(data_rate);
        self
    }

    /// Sets bandwidth.
    ///
    /// Use [`data_rate()`](ItemBuilder::data_rate()) with [`DataRate`] for predefined options.
    pub fn raw_bandwidth(&mut self, bandwidth: Bandwidth) -> &mut Self {
        let bandwidth = match bandwidth {
            Bandwidth::Bw125 => 125_000,
            Bandwidth::Bw250 => 250_000,
        };
        self.bandwidth = Some(bandwidth);
        self
    }

    /// Sets spreading factor.
    ///
    /// Use [`data_rate()`](ItemBuilder::data_rate()) with [`DataRate`] for predefined options.
    pub fn raw_spreading_factor(&mut self, spreading_factor: SpreadingFactor) -> &mut Self {
        self.spreading_factor = Some(spreading_factor as u32);
        self
    }

    /// Sets code rate.
    ///
    /// Defaults to `4/5`.
    pub fn code_rate(&mut self, code_rate: CodingRate) -> &mut Self {
        let code_rate = match code_rate {
            CodingRate::Cr45 => chirpstack_api::gw::CodeRate::Cr45,
        };
        self.code_rate = Some(code_rate);
        self
    }

    /// Sets polarization inversion.
    ///
    /// Defaults to `false`.
    pub fn polarization_inversion(&mut self, polarization_inversion: bool) -> &mut Self {
        self.polarization_inversion = Some(polarization_inversion);
        self
    }

    /// Sets board.
    pub fn board(&mut self, board: u32) -> &mut Self {
        self.board = Some(board);
        self
    }

    /// Sets antenna.
    pub fn antenna(&mut self, antenna: u32) -> &mut Self {
        self.antenna = Some(antenna);
        self
    }
    /// Sets downlink context.
    pub fn context(&mut self, context: Vec<u8>) -> &mut Self {
        self.context = Some(context);
        self
    }

    /// Builds [`DownlinkItem`] with base parameters (shared by all variants).
    fn build_base(&mut self) -> Result<DownlinkItem<Dt>, DownlinkItemBuilderError>
    where
        Dt: DownlinkType,
    {
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
                downlink_type: PhantomData::<Dt>,
            },
        })
    }

    /// Checks whether the set parameters are plausible.
    ///
    /// Payload size checking is only available if [`data_rate`](DownlinkItemBuilder.data_rate) is set.
    fn check_base_for_plausibility(&self) -> Result<(), DownlinkItemBuilderError> {
        if self.phy_payload.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "phy_payload".to_owned(),
            });
        }
        if self.frequency.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "frequency".to_owned(),
            });
        }
        if self.power.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "power".to_owned(),
            });
        }
        if self.bandwidth.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "bandwidth".to_owned(),
            });
        }
        if self.spreading_factor.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "spreading_factor".to_owned(),
            });
        }
        if self.code_rate.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "code_rate".to_owned(),
            });
        }
        if self.polarization_inversion.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "polarization_inverse".to_owned(),
            });
        }
        if self.board.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "board".to_owned(),
            });
        }
        if self.antenna.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
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
                )?;
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
    /// Creates a new [`DownlinkItemBuilder<DelayTimingClassA>`].
    #[must_use]
    pub fn new() -> Self {
        DownlinkItemBuilder::default()
    }

    /// Sets downlink delay.
    pub fn delay(&mut self, delay: std::time::Duration) -> &mut Self {
        self.delay = Some(delay);
        self
    }

    /// Checks whether the set parameters are plausible.
    ///
    /// # Errors
    ///
    /// Returns an error if a parameter is missing of if the payload is too big (only if payload size
    /// checking is active).
    pub fn check_for_plausibility(&self) -> Result<(), DownlinkItemBuilderError> {
        self.check_base_for_plausibility()?;
        if self.delay.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "delay".to_owned(),
            });
        }

        Ok(())
    }

    /// Builds [`DownlinkItem`].
    ///
    /// # Errors
    ///
    /// Returns an error if a parameter is missing of if the payload is too big (only if payload size
    /// checking is active).
    pub fn build(&mut self) -> Result<DownlinkItem<DelayTimingClassA>, DownlinkItemBuilderError> {
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
    /// Creates a new [`DownlinkItemBuilder<GpsTimingClassB>`].
    #[must_use]
    pub fn new() -> Self {
        DownlinkItemBuilder::default()
    }

    /// Sets time since GPS epoch.
    pub fn time_since_gps_epoch(&mut self, time_since_gps_epoch: std::time::Duration) -> &mut Self {
        self.time_since_gps_epoch = Some(time_since_gps_epoch);
        self
    }

    /// Checks whether the set parameters are plausible.
    ///
    /// # Errors
    ///
    /// Returns an error if a parameter is missing of if the payload is too big (only if payload size
    /// checking is active).
    pub fn check_for_plausibility(&self) -> Result<(), DownlinkItemBuilderError> {
        self.check_base_for_plausibility()?;
        if self.time_since_gps_epoch.is_none() {
            return Err(DownlinkItemBuilderError::MissingParameter {
                missing: "time_since_gps_epoch".to_owned(),
            });
        }
        Ok(())
    }

    /// Builds [`DownlinkItem`].
    ///
    /// # Errors
    ///
    /// Returns an error if a parameter is missing of if the payload is too big (only if payload size
    /// checking is active).
    pub fn build(&mut self) -> Result<DownlinkItem<GpsTimingClassB>, DownlinkItemBuilderError> {
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
    /// Creates a new [`DownlinkItemBuilder<ImmediatelyClassC>`].
    #[must_use]
    pub fn new() -> Self {
        DownlinkItemBuilder::default()
    }

    /// Checks whether the set parameters are plausible.
    ///
    /// # Errors
    ///
    /// Returns an error if a parameter is missing of if the payload is too big (only if payload size
    /// checking is active).
    pub fn check_for_plausibility(&self) -> Result<(), DownlinkItemBuilderError> {
        self.check_base_for_plausibility()?;
        Ok(())
    }

    /// Builds [`DownlinkItem`].
    ///
    /// # Errors
    ///
    /// Returns an error if a parameter is missing of if the payload is too big (only if payload size
    /// checking is active).
    pub fn build(&mut self) -> Result<DownlinkItem<ImmediatelyClassC>, DownlinkItemBuilderError> {
        self.check_for_plausibility()?;
        self.build_base()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chirpstack_api::gw::CodeRate;
    use std::time::Duration;

    #[test]
    fn test_downlink_item_builder_delay_timing_class_a() {
        let payload = Vec::new();
        let frequency = Frequency::Freq868_1;
        let power = 3;
        let bandwidth = Bandwidth::Bw125;
        let spreading_factor = SpreadingFactor::SF12;
        let polarization_inversion = false;
        let board = 3;
        let antenna = 1;
        let delay = Duration::from_secs(1);

        let mut builder = DownlinkItemBuilder::<DelayTimingClassA>::new();
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "phy_payload".to_owned(),
            }),
            builder.build()
        );
        builder.phy_payload(payload.clone());
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "frequency".to_owned(),
            }),
            builder.build()
        );
        builder.frequency(frequency);
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "power".to_owned(),
            }),
            builder.build()
        );
        builder.power(power);
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "bandwidth".to_owned(),
            }),
            builder.build()
        );
        builder.raw_bandwidth(bandwidth);
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "spreading_factor".to_owned(),
            }),
            builder.build()
        );
        builder.raw_spreading_factor(spreading_factor);
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "board".to_owned(),
            }),
            builder.build()
        );
        builder.board(board);
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "antenna".to_owned(),
            }),
            builder.build()
        );
        builder.antenna(antenna);
        assert_eq!(
            Err(DownlinkItemBuilderError::MissingParameter {
                missing: "delay".to_owned(),
            }),
            builder.build()
        );
        builder.delay(delay);
        let item = DownlinkItem::<DelayTimingClassA> {
            phy_payload: payload,
            tx_info: TxInfo {
                frequency: 868_100_000,
                power,
                lo_ra_modulation_info: LoRaModulationInfo {
                    bandwidth: bandwidth.hz(),
                    spreading_factor: spreading_factor.into(),
                    code_rate: CodeRate::Cr45,
                    polarization_inversion,
                },
                board,
                antenna,
                delay_timing_info: Some(DelayTimingInfo { delay }),
                context: Some(Vec::new()),
                gps_epoch_timing_info: None,
                downlink_type: PhantomData::<DelayTimingClassA>,
            },
        };
        assert_eq!(Ok(item), builder.build());
    }
}
