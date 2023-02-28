//! Collection of predefined LoRaWan parameters and helper functions.

use crate::error::{
    BandwidthConversionError, DataRateConversionError, DownlinkError,
    SpreadingFactorConversionError,
};

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
    pub fn khz(&self) -> u32 {
        match self {
            Bandwidth::Bw125 => 125,
            Bandwidth::Bw250 => 250,
        }
    }

    /// Try to convert from `u32` to [`Bandwidth`]. Expects value in kHz.
    pub fn try_from_khz(bandwidth: u32) -> Result<Self, BandwidthConversionError> {
        match bandwidth {
            125 => Ok(Bandwidth::Bw125),
            250 => Ok(Bandwidth::Bw250),
            _ => Err(BandwidthConversionError::NoSuchBandwidth { bandwidth }),
        }
    }

    /// Bandwidth in Hz.
    pub fn hz(&self) -> u32 {
        match self {
            Bandwidth::Bw125 => 125000,
            Bandwidth::Bw250 => 250000,
        }
    }
    /// Try to convert from `u32` to [`Bandwidth`]. Expects value in Hz.
    pub fn try_from_hz(bandwidth: u32) -> Result<Self, BandwidthConversionError> {
        match bandwidth {
            125000 => Ok(Bandwidth::Bw125),
            250000 => Ok(Bandwidth::Bw250),
            _ => Err(BandwidthConversionError::NoSuchBandwidth { bandwidth }),
        }
    }
}

/// Bandwidth
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum CodingRate {
    /// Coding rate of 4/5
    Cr45,
}

impl CodingRate {
    /// The value corresponding to the coding rate used in the airtime calculations.
    pub fn value_for_airtime_cal(&self) -> u32 {
        match self {
            CodingRate::Cr45 => 1,
        }
    }
}

/// Data rates.
///DR0-DR5 required by LoRa standard for end devices and gateways.
#[allow(missing_docs)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum DataRate {
    Eu863_870Dr0,
    Eu863_870Dr1,
    Eu863_870Dr2,
    Eu863_870Dr3,
    Eu863_870Dr4,
    Eu863_870Dr5,
    Eu863_870Dr6,
}

/// Frequencies required by LoRa standard for end devices and gateways
#[allow(missing_docs)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Frequency {
    Freq868_1,
    Freq868_3,
    Freq868_5,
}

impl DataRate {
    /// Returns the maximum payload (PHYPayload) size for a given [`DataRate`].
    /// Repeater compatability might reduce the maximum payload size.
    pub fn max_allowed_payload_size(&self, repeater_compatible: bool) -> u8 {
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
    /// This excludes the MHDR part of the payload.
    /// Repeater compatability might reduce the maximum payload size.
    pub fn max_usable_payload_size(&self, repeater_compatible: bool) -> u8 {
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
    pub fn check_payload_size(&self, payload_size: usize) -> Result<(), DownlinkError> {
        let max_payload_size = self.max_allowed_payload_size(false);
        if payload_size > max_payload_size as usize {
            return Err(DownlinkError::PayloadTooBig {
                over_limit: payload_size - max_payload_size as usize,
            });
        }
        Ok(())
    }

    /// Attempts to convert the provided bandwidth and spreading factor into a data rate.
    pub fn from_raw_bandwidth_and_spreading_factor(
        bandwidth: u32,
        spreading_factor: u32,
    ) -> Result<Self, DataRateConversionError> {
        match (bandwidth, spreading_factor) {
            (125000, 12) => Ok(Self::Eu863_870Dr0),
            (125000, 11) => Ok(Self::Eu863_870Dr1),
            (125000, 10) => Ok(Self::Eu863_870Dr2),
            (125000, 9) => Ok(Self::Eu863_870Dr3),
            (125000, 8) => Ok(Self::Eu863_870Dr4),
            (125000, 7) => Ok(Self::Eu863_870Dr5),
            (250000, 7) => Ok(Self::Eu863_870Dr6),
            _ => Err(DataRateConversionError::WrongParameters {
                bandwidth,
                spreading_factor,
            }),
        }
    }

    /// Attempts to convert the provided bandwidth and spreading factor into a data rate.
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

    /// Returns bandwidth and spreading factor for the data rate.
    /// Returns: (bandwidth, spreading_factor)
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

    /// Returns bandwidth and spreading factor for the data rate.
    /// Returns: (bandwidth, spreading_factor)
    pub fn into_raw_bandwidth_and_spreading_factor(self) -> (u32, u32) {
        match self {
            DataRate::Eu863_870Dr0 => (125000, 12),
            DataRate::Eu863_870Dr1 => (125000, 11),
            DataRate::Eu863_870Dr2 => (125000, 10),
            DataRate::Eu863_870Dr3 => (125000, 9),
            DataRate::Eu863_870Dr4 => (125000, 8),
            DataRate::Eu863_870Dr5 => (125000, 7),
            DataRate::Eu863_870Dr6 => (250000, 7),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_within_limit() {
        let payload: Vec<u8> = vec![0xFF; 30];
        let data_rate = DataRate::Eu863_870Dr0;
        let result = data_rate.check_payload_size(payload.len());
        assert!(result.is_ok());
    }

    #[test]
    fn test_payload_equal_limit() {
        let payload: Vec<u8> = vec![0xFF; 63];
        let data_rate = DataRate::Eu863_870Dr0;
        let result = data_rate.check_payload_size(payload.len());
        assert!(result.is_ok());
    }
    #[test]
    fn test_payload_over_limit() {
        let payload: Vec<u8> = vec![0xFF; 70];
        let data_rate = DataRate::Eu863_870Dr0;
        let result = data_rate.check_payload_size(payload.len());
        assert!(result.is_err());
        if let Some(DownlinkError::PayloadTooBig { over_limit }) = result.err() {
            assert_eq!(over_limit, 6)
        }
    }
}
