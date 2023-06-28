//! Collection of predefined LoRaWan parameters and helper functions.

use crate::error::{
    BandwidthConversionError, DataRateConversionError, DownlinkItemBuilderError,
    SpreadingFactorConversionError,
};
use serde_derive::{Deserialize, Serialize};

/// Minimal physical payload size, 7 bytes from MACPayload, 4 bytes from MIC
pub const MIN_PHY_PAYLOAD: usize = 7 + 4;

/// Spreading factor
#[allow(missing_docs)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum SpreadingFactor {
    SF7 = 7,
    SF8 = 8,
    SF9 = 9,
    SF10 = 10,
    SF11 = 11,
    SF12 = 12,
}

impl From<SpreadingFactor> for u32 {
    fn from(spreading_factor: SpreadingFactor) -> Self {
        match spreading_factor {
            SpreadingFactor::SF7 => 7,
            SpreadingFactor::SF8 => 8,
            SpreadingFactor::SF9 => 9,
            SpreadingFactor::SF10 => 10,
            SpreadingFactor::SF11 => 11,
            SpreadingFactor::SF12 => 12,
        }
    }
}

impl TryFrom<u32> for SpreadingFactor {
    type Error = SpreadingFactorConversionError;

    fn try_from(spreading_factor: u32) -> Result<Self, Self::Error> {
        match spreading_factor {
            7 => Ok(SpreadingFactor::SF7),
            8 => Ok(SpreadingFactor::SF8),
            9 => Ok(SpreadingFactor::SF9),
            10 => Ok(SpreadingFactor::SF10),
            11 => Ok(SpreadingFactor::SF11),
            12 => Ok(SpreadingFactor::SF12),
            _ => Err(SpreadingFactorConversionError::NoSuchSpreadingFactor { spreading_factor }),
        }
    }
}

/// Bandwidth
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Bandwidth {
    /// 125kHz
    Bw125,
    /// 250kHz
    Bw250,
}

impl Bandwidth {
    /// Bandwidth in kHz.
    #[must_use]
    pub fn khz(&self) -> u32 {
        match self {
            Bandwidth::Bw125 => 125,
            Bandwidth::Bw250 => 250,
        }
    }

    /// Tries to convert from `u32` to [`Bandwidth`].
    ///
    /// Expects value in kHz.
    ///
    /// # Errors
    ///
    /// Returns an error if the provided bandwidth is neither 125 nor 250.
    pub fn try_from_khz(bandwidth: u32) -> Result<Self, BandwidthConversionError> {
        match bandwidth {
            125 => Ok(Bandwidth::Bw125),
            250 => Ok(Bandwidth::Bw250),
            _ => Err(BandwidthConversionError::NoSuchBandwidth { bandwidth }),
        }
    }

    /// Bandwidth in Hz.
    #[must_use]
    pub fn hz(&self) -> u32 {
        match self {
            Bandwidth::Bw125 => 125_000,
            Bandwidth::Bw250 => 250_000,
        }
    }
    /// Tries to convert from `u32` to [`Bandwidth`].
    ///
    /// Expects value in Hz.
    ///
    /// # Errors
    ///
    /// Returns an error if the provided bandwidth is neither 125000 nor 250000.
    pub fn try_from_hz(bandwidth: u32) -> Result<Self, BandwidthConversionError> {
        match bandwidth {
            125_000 => Ok(Bandwidth::Bw125),
            250_000 => Ok(Bandwidth::Bw250),
            _ => Err(BandwidthConversionError::NoSuchBandwidth { bandwidth }),
        }
    }
}

/// Coding rate
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum CodingRate {
    /// Coding rate of 4/5
    Cr45,
}

impl CodingRate {
    /// The value corresponding to the coding rate used in the airtime calculations.
    ///
    /// See "Semtech AN1200.13 LoRa Modem Designer's Guide" for details.
    #[must_use]
    pub fn value_for_airtime_cal(&self) -> u32 {
        match self {
            CodingRate::Cr45 => 1,
        }
    }
}

/// Data rates.
/// DR0-DR5 required by LoRa standard for end devices and gateways.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum DataRate {
    Eu863_870Dr0,
    Eu863_870Dr1,
    Eu863_870Dr2,
    Eu863_870Dr3,
    Eu863_870Dr4,
    Eu863_870Dr5,
    Eu863_870Dr6,
}

/// Frequencies required by LoRa standard for end devices and gateways.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Frequency {
    Freq868_1,
    Freq868_3,
    Freq868_5,
}

impl DataRate {
    /// Returns the maximum payload (PHYPayload) size for a given [`DataRate`].
    ///
    /// Repeater compatability might reduce the maximum payload size.
    #[must_use]
    pub fn max_allowed_payload_size(&self, repeater_compatible: bool) -> usize {
        // All payload are calculated from maximum MHDR + MACPayload + MIC
        // (see "TS001-1.0.4 LoRaWAN® L2 1.0.4 Specification" and "RP002-1.0.3 LoRaWAN® Regional Parameters")
        match self {
            DataRate::Eu863_870Dr0 | DataRate::Eu863_870Dr1 | DataRate::Eu863_870Dr2 => 1 + 59 + 4,
            DataRate::Eu863_870Dr3 => 1 + 123 + 4,
            DataRate::Eu863_870Dr4 | DataRate::Eu863_870Dr5 | DataRate::Eu863_870Dr6 => {
                if repeater_compatible {
                    1 + 230 + 4
                } else {
                    1 + 250 + 4
                }
            }
        }
    }

    /// Returns the maximum usable payload (PHYPayload) size for a given [`DataRate`].
    ///
    /// This excludes the MHDR part of the payload.
    /// Repeater compatability might reduce the maximum payload size.
    #[must_use]
    pub fn max_usable_payload_size(&self, repeater_compatible: bool) -> usize {
        // All payload are calculated from maximum MACPayload + MIC
        // (see "TS001-1.0.4 LoRaWAN® L2 1.0.4 Specification" and "RP002-1.0.3 LoRaWAN® Regional Parameters")
        match self {
            DataRate::Eu863_870Dr0 | DataRate::Eu863_870Dr1 | DataRate::Eu863_870Dr2 => 59 + 4,
            DataRate::Eu863_870Dr3 => 123 + 4,
            DataRate::Eu863_870Dr4 | DataRate::Eu863_870Dr5 | DataRate::Eu863_870Dr6 => {
                if repeater_compatible {
                    230 + 4
                } else {
                    250 + 4
                }
            }
        }
    }

    /// Checks whether the supplied payload is within the allowed payload size for the specified
    /// data rate.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is too big for the data rate.
    pub fn check_payload_size(&self, payload_size: usize) -> Result<(), DownlinkItemBuilderError> {
        let max_payload_size = self.max_allowed_payload_size(false);
        if payload_size > max_payload_size {
            return Err(DownlinkItemBuilderError::PayloadTooBig {
                over_limit: payload_size - max_payload_size,
            });
        }
        Ok(())
    }

    /// Attempts to convert the provided bandwidth and spreading factor into a data rate.
    ///
    /// # Errors
    ///
    /// Returns an error if the parameter combination is not valid.
    pub fn from_raw_bandwidth_and_spreading_factor(
        bandwidth: u32,
        spreading_factor: u32,
    ) -> Result<Self, DataRateConversionError> {
        match (bandwidth, spreading_factor) {
            (125_000, 12) => Ok(Self::Eu863_870Dr0),
            (125_000, 11) => Ok(Self::Eu863_870Dr1),
            (125_000, 10) => Ok(Self::Eu863_870Dr2),
            (125_000, 9) => Ok(Self::Eu863_870Dr3),
            (125_000, 8) => Ok(Self::Eu863_870Dr4),
            (125_000, 7) => Ok(Self::Eu863_870Dr5),
            (250_000, 7) => Ok(Self::Eu863_870Dr6),
            _ => Err(DataRateConversionError::WrongParameters {
                bandwidth,
                spreading_factor,
            }),
        }
    }

    /// Attempts to convert the provided bandwidth and spreading factor into a data rate.
    ///
    /// # Errors
    ///
    /// Returns an error if the parameter combination is not valid.
    pub fn from_bandwidth_and_spreading_factor(
        bandwidth: Bandwidth,
        spreading_factor: SpreadingFactor,
    ) -> Result<Self, DataRateConversionError> {
        match (bandwidth, spreading_factor) {
            (Bandwidth::Bw125, SpreadingFactor::SF12) => Ok(Self::Eu863_870Dr0),
            (Bandwidth::Bw125, SpreadingFactor::SF11) => Ok(Self::Eu863_870Dr1),
            (Bandwidth::Bw125, SpreadingFactor::SF10) => Ok(Self::Eu863_870Dr2),
            (Bandwidth::Bw125, SpreadingFactor::SF9) => Ok(Self::Eu863_870Dr3),
            (Bandwidth::Bw125, SpreadingFactor::SF8) => Ok(Self::Eu863_870Dr4),
            (Bandwidth::Bw125, SpreadingFactor::SF7) => Ok(Self::Eu863_870Dr5),
            (Bandwidth::Bw250, SpreadingFactor::SF7) => Ok(Self::Eu863_870Dr6),
            _ => Err(DataRateConversionError::WrongParameters {
                bandwidth: bandwidth.hz(),
                spreading_factor: spreading_factor as u32,
            }),
        }
    }

    /// Returns typed bandwidth and spreading factor corresponding to the data rate.
    #[must_use]
    pub fn into_bandwidth_and_spreading_factor(self) -> (Bandwidth, SpreadingFactor) {
        match self {
            DataRate::Eu863_870Dr0 => (Bandwidth::Bw125, SpreadingFactor::SF12),
            DataRate::Eu863_870Dr1 => (Bandwidth::Bw125, SpreadingFactor::SF11),
            DataRate::Eu863_870Dr2 => (Bandwidth::Bw125, SpreadingFactor::SF10),
            DataRate::Eu863_870Dr3 => (Bandwidth::Bw125, SpreadingFactor::SF9),
            DataRate::Eu863_870Dr4 => (Bandwidth::Bw125, SpreadingFactor::SF8),
            DataRate::Eu863_870Dr5 => (Bandwidth::Bw125, SpreadingFactor::SF7),
            DataRate::Eu863_870Dr6 => (Bandwidth::Bw250, SpreadingFactor::SF7),
        }
    }

    /// Returns bandwidth and spreading factor corresponding to the data rate as [`u32`].
    ///
    /// Returns: (bandwidth, spreading_factor)
    #[must_use]
    pub fn into_raw_bandwidth_and_spreading_factor(self) -> (u32, u32) {
        match self {
            DataRate::Eu863_870Dr0 => (125_000, 12),
            DataRate::Eu863_870Dr1 => (125_000, 11),
            DataRate::Eu863_870Dr2 => (125_000, 10),
            DataRate::Eu863_870Dr3 => (125_000, 9),
            DataRate::Eu863_870Dr4 => (125_000, 8),
            DataRate::Eu863_870Dr5 => (125_000, 7),
            DataRate::Eu863_870Dr6 => (250_000, 7),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spreading_factor_from() {
        assert_eq!(7, u32::from(SpreadingFactor::SF7));
        assert_eq!(8, u32::from(SpreadingFactor::SF8));
        assert_eq!(9, u32::from(SpreadingFactor::SF9));
        assert_eq!(10, u32::from(SpreadingFactor::SF10));
        assert_eq!(11, u32::from(SpreadingFactor::SF11));
        assert_eq!(12, u32::from(SpreadingFactor::SF12));
    }

    #[test]
    fn test_data_rate_payload_within_limit() {
        let payload: Vec<u8> = vec![0xFF; 30];
        let data_rate = DataRate::Eu863_870Dr0;
        let result = data_rate.check_payload_size(payload.len());
        assert!(result.is_ok());
    }

    #[test]
    fn test_data_rate_payload_equal_limit() {
        let payload: Vec<u8> = vec![0xFF; 63];
        let data_rate = DataRate::Eu863_870Dr0;
        let result = data_rate.check_payload_size(payload.len());
        assert!(result.is_ok());
    }
    #[test]
    fn test_data_rate_payload_over_limit() {
        let payload: Vec<u8> = vec![0xFF; 70];
        let data_rate = DataRate::Eu863_870Dr0;
        let result = data_rate.check_payload_size(payload.len());
        assert!(result.is_err());
        if let Some(DownlinkItemBuilderError::PayloadTooBig { over_limit }) = result.err() {
            assert_eq!(over_limit, 6);
        }
    }

    #[test]
    fn test_spreading_factor_try_from() {
        assert_eq!(Ok(SpreadingFactor::SF7), SpreadingFactor::try_from(7));
        assert_eq!(Ok(SpreadingFactor::SF8), SpreadingFactor::try_from(8));
        assert_eq!(Ok(SpreadingFactor::SF9), SpreadingFactor::try_from(9));
        assert_eq!(Ok(SpreadingFactor::SF10), SpreadingFactor::try_from(10));
        assert_eq!(Ok(SpreadingFactor::SF11), SpreadingFactor::try_from(11));
        assert_eq!(Ok(SpreadingFactor::SF12), SpreadingFactor::try_from(12));

        assert_eq!(
            Err(SpreadingFactorConversionError::NoSuchSpreadingFactor {
                spreading_factor: 1
            }),
            SpreadingFactor::try_from(1)
        );
    }

    #[test]
    fn test_bandwidth_khz() {
        assert_eq!(125, Bandwidth::Bw125.khz());
        assert_eq!(250, Bandwidth::Bw250.khz());
    }

    #[test]
    fn test_bandwidth_hz() {
        assert_eq!(125_000, Bandwidth::Bw125.hz());
        assert_eq!(250_000, Bandwidth::Bw250.hz());
    }

    #[test]
    fn test_bandwidth_try_from_khz() {
        assert_eq!(Ok(Bandwidth::Bw125), Bandwidth::try_from_khz(125));
        assert_eq!(Ok(Bandwidth::Bw250), Bandwidth::try_from_khz(250));

        assert_eq!(
            Err(BandwidthConversionError::NoSuchBandwidth { bandwidth: 123 }),
            Bandwidth::try_from_khz(123)
        );
    }

    #[test]
    fn test_bandwidth_try_from_hz() {
        assert_eq!(Ok(Bandwidth::Bw125), Bandwidth::try_from_hz(125_000));
        assert_eq!(Ok(Bandwidth::Bw250), Bandwidth::try_from_hz(250_000));

        assert_eq!(
            Err(BandwidthConversionError::NoSuchBandwidth { bandwidth: 123 }),
            Bandwidth::try_from_hz(123)
        );
    }

    #[test]
    fn test_data_rate_max_allowed_payload_size() {
        assert_eq!(64, DataRate::Eu863_870Dr0.max_allowed_payload_size(false));
        assert_eq!(64, DataRate::Eu863_870Dr0.max_allowed_payload_size(true));

        assert_eq!(64, DataRate::Eu863_870Dr1.max_allowed_payload_size(false));
        assert_eq!(64, DataRate::Eu863_870Dr1.max_allowed_payload_size(true));

        assert_eq!(64, DataRate::Eu863_870Dr2.max_allowed_payload_size(false));
        assert_eq!(64, DataRate::Eu863_870Dr2.max_allowed_payload_size(true));

        assert_eq!(128, DataRate::Eu863_870Dr3.max_allowed_payload_size(false));
        assert_eq!(128, DataRate::Eu863_870Dr3.max_allowed_payload_size(true));

        assert_eq!(255, DataRate::Eu863_870Dr4.max_allowed_payload_size(false));
        assert_eq!(235, DataRate::Eu863_870Dr4.max_allowed_payload_size(true));

        assert_eq!(255, DataRate::Eu863_870Dr5.max_allowed_payload_size(false));
        assert_eq!(235, DataRate::Eu863_870Dr5.max_allowed_payload_size(true));

        assert_eq!(255, DataRate::Eu863_870Dr6.max_allowed_payload_size(false));
        assert_eq!(235, DataRate::Eu863_870Dr6.max_allowed_payload_size(true));
    }

    #[test]
    fn test_data_rate_max_usable_payload_size() {
        assert_eq!(63, DataRate::Eu863_870Dr0.max_usable_payload_size(false));
        assert_eq!(63, DataRate::Eu863_870Dr0.max_usable_payload_size(true));

        assert_eq!(63, DataRate::Eu863_870Dr1.max_usable_payload_size(false));
        assert_eq!(63, DataRate::Eu863_870Dr1.max_usable_payload_size(true));

        assert_eq!(63, DataRate::Eu863_870Dr2.max_usable_payload_size(false));
        assert_eq!(63, DataRate::Eu863_870Dr2.max_usable_payload_size(true));

        assert_eq!(127, DataRate::Eu863_870Dr3.max_usable_payload_size(false));
        assert_eq!(127, DataRate::Eu863_870Dr3.max_usable_payload_size(true));

        assert_eq!(254, DataRate::Eu863_870Dr4.max_usable_payload_size(false));
        assert_eq!(234, DataRate::Eu863_870Dr4.max_usable_payload_size(true));

        assert_eq!(254, DataRate::Eu863_870Dr5.max_usable_payload_size(false));
        assert_eq!(234, DataRate::Eu863_870Dr5.max_usable_payload_size(true));

        assert_eq!(254, DataRate::Eu863_870Dr6.max_usable_payload_size(false));
        assert_eq!(234, DataRate::Eu863_870Dr6.max_usable_payload_size(true));
    }

    #[test]
    fn test_data_rate_from_raw_bandwidth_and_spreading_factor() {
        assert_eq!(
            Ok(DataRate::Eu863_870Dr0),
            DataRate::from_raw_bandwidth_and_spreading_factor(125_000, 12)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr1),
            DataRate::from_raw_bandwidth_and_spreading_factor(125_000, 11)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr2),
            DataRate::from_raw_bandwidth_and_spreading_factor(125_000, 10)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr3),
            DataRate::from_raw_bandwidth_and_spreading_factor(125_000, 9)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr4),
            DataRate::from_raw_bandwidth_and_spreading_factor(125_000, 8)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr5),
            DataRate::from_raw_bandwidth_and_spreading_factor(125_000, 7)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr6),
            DataRate::from_raw_bandwidth_and_spreading_factor(250_000, 7)
        );

        assert_eq!(
            Err(DataRateConversionError::WrongParameters {
                bandwidth: 250_000,
                spreading_factor: 8,
            }),
            DataRate::from_raw_bandwidth_and_spreading_factor(250_000, 8)
        );
    }

    #[test]
    fn test_data_rate_from_bandwidth_and_spreading_factor() {
        assert_eq!(
            Ok(DataRate::Eu863_870Dr0),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw125, SpreadingFactor::SF12)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr1),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw125, SpreadingFactor::SF11)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr2),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw125, SpreadingFactor::SF10)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr3),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw125, SpreadingFactor::SF9)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr4),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw125, SpreadingFactor::SF8)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr5),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw125, SpreadingFactor::SF7)
        );

        assert_eq!(
            Ok(DataRate::Eu863_870Dr6),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw250, SpreadingFactor::SF7)
        );

        assert_eq!(
            Err(DataRateConversionError::WrongParameters {
                bandwidth: 250_000,
                spreading_factor: 8,
            }),
            DataRate::from_bandwidth_and_spreading_factor(Bandwidth::Bw250, SpreadingFactor::SF8)
        );
    }

    #[test]
    fn test_data_rate_into_bandwidth_and_spreading_factor() {
        assert_eq!(
            (Bandwidth::Bw125, SpreadingFactor::SF12),
            DataRate::Eu863_870Dr0.into_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (Bandwidth::Bw125, SpreadingFactor::SF11),
            DataRate::Eu863_870Dr1.into_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (Bandwidth::Bw125, SpreadingFactor::SF10),
            DataRate::Eu863_870Dr2.into_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (Bandwidth::Bw125, SpreadingFactor::SF9),
            DataRate::Eu863_870Dr3.into_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (Bandwidth::Bw125, SpreadingFactor::SF8),
            DataRate::Eu863_870Dr4.into_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (Bandwidth::Bw125, SpreadingFactor::SF7),
            DataRate::Eu863_870Dr5.into_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (Bandwidth::Bw250, SpreadingFactor::SF7),
            DataRate::Eu863_870Dr6.into_bandwidth_and_spreading_factor()
        );
    }

    #[test]
    fn test_data_rate_into_raw_bandwidth_and_spreading_factor() {
        assert_eq!(
            (125_000, 12),
            DataRate::Eu863_870Dr0.into_raw_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (125_000, 11),
            DataRate::Eu863_870Dr1.into_raw_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (125_000, 10),
            DataRate::Eu863_870Dr2.into_raw_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (125_000, 9),
            DataRate::Eu863_870Dr3.into_raw_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (125_000, 8),
            DataRate::Eu863_870Dr4.into_raw_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (125_000, 7),
            DataRate::Eu863_870Dr5.into_raw_bandwidth_and_spreading_factor()
        );

        assert_eq!(
            (250_000, 7),
            DataRate::Eu863_870Dr6.into_raw_bandwidth_and_spreading_factor()
        );
    }
}
