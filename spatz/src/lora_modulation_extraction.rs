//! Extraction of modulation info from ChirpStack frames.

use crate::error::LoRaModulationExtractionError;
use chirpstack_api::gw::{modulation, DownlinkTxInfo, LoraModulationInfo, UplinkTxInfo};
use tracing::error;

/// Extract [`LoraModulationInfo`](chirpstack_api::gw::LoraModulationInfo) and frequency
/// from [`DownlinkTxInfo`](chirpstack_api::gw::DownlinkTxInfo).
///
/// # Errors
///
/// Returns an error if:
/// - there is no tx info.
/// - there is no modulation info.
/// - there are no LoRa parameters.
pub fn extract_modulation_freq_info_from_downlink_tx_info(
    tx_info: Option<DownlinkTxInfo>,
) -> Result<(u32, LoraModulationInfo), LoRaModulationExtractionError> {
    if let Some(tx_info) = tx_info {
        let freq = tx_info.frequency;
        if let Some(modulation) = tx_info.modulation {
            if let Some(modulation::Parameters::Lora(lora_modulation_info)) = modulation.parameters
            {
                Ok((freq, lora_modulation_info))
            } else {
                let err = LoRaModulationExtractionError::NoLoRaParameters;
                error!(%err);
                Err(err)
            }
        } else {
            let err = LoRaModulationExtractionError::NoModulationInfo;
            error!(%err);
            Err(err)
        }
    } else {
        let err = LoRaModulationExtractionError::NoTxInfo;
        error!(%err);
        Err(err)
    }
}

/// Extract [`LoraModulationInfo`](chirpstack_api::gw::LoraModulationInfo) from [`UplinkTxInfo`](chirpstack_api::gw::UplinkTxInfo).
///
/// # Errors
///
/// Returns an error if:
/// - there is no tx info.
/// - there is no modulation info.
/// - there are no LoRa parameters.
pub fn extract_modulation_info_from_uplink_tx_info(
    tx_info: Option<UplinkTxInfo>,
) -> Result<LoraModulationInfo, LoRaModulationExtractionError> {
    if let Some(tx_info) = tx_info {
        if let Some(modulation) = tx_info.modulation {
            if let Some(modulation::Parameters::Lora(lora_modulation_info)) = modulation.parameters
            {
                Ok(lora_modulation_info)
            } else {
                Err(LoRaModulationExtractionError::NoLoRaParameters)
            }
        } else {
            Err(LoRaModulationExtractionError::NoModulationInfo)
        }
    } else {
        Err(LoRaModulationExtractionError::NoTxInfo)
    }
}
