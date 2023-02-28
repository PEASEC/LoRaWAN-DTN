//! Calculations taken from "Semtech AN1200.13 LoRa Modem Designer's Guide"
//! LoRaWAN values taken from "LoRaWAN® Regional Parameters RP002-1.0.4"

use crate::error::AirtimeCalculationError;
use crate::lora_modulation_extraction::extract_modulation_freq_info_from_downlink_tx_info;
use chirpstack_api::gw::LoraModulationInfo;
use chirpstack_gwb_integration::downlinks::predefined_parameters::{
    Bandwidth, CodingRate, SpreadingFactor,
};

static LORA_PREAMBLE_LENGTH_EU868_870_IN_SYMBOLS: f64 = 8.0;
static LORA_SYNC_WORD_LENGTH_IN_SYMBOLS: f64 = 4.25;

/// T_sym as described in chapter 4 "Semtech AN1200.13 LoRa Modem Designer's Guide"
/// `bandwidth` as x kHz (e.g. 250 kHz -> `bandwidth` = 250)
fn symbol_duration(spreading_factor: SpreadingFactor, bandwidth: Bandwidth) -> f64 {
    2.0_f64.powf(f64::from(spreading_factor as u32)) / f64::from(bandwidth.khz())
}

/// T_preamble as described in chapter 4 "Semtech AN1200.13 LoRa Modem Designer's Guide"
fn preamble_duration(
    preamble_len_symbols: f64,
    sync_word_len_symbol: f64,
    symbol_duration: f64,
) -> f64 {
    (preamble_len_symbols + sync_word_len_symbol) * symbol_duration
}

/// payloadSymbNb as described in chapter 4 "Semtech AN1200.13 LoRa Modem Designer's Guide"
/// If `is_uplink` is `true`, the payload crc is included, if not, it is removed.
/// The 16 bits from the equation are assumed to be the payload crc part.
fn payload_symbols(
    phy_payload_len_bytes: u32,
    spreading_factor: SpreadingFactor,
    header_disabled: bool,
    data_rate_optimization_enabled: bool,
    coding_rate: CodingRate,
    is_uplink: bool,
) -> u32 {
    let phy_payload_len_bytes = f64::from(phy_payload_len_bytes);
    let spreading_factor = f64::from(spreading_factor as u32);
    let header_disabled = f64::from(u32::from(header_disabled));
    let data_rate_optimization_enabled = f64::from(u32::from(data_rate_optimization_enabled));
    let coding_rate = coding_rate.value_for_airtime_cal();
    let is_uplink = f64::from(is_uplink as u32);
    ((((8.0 * phy_payload_len_bytes - 4.0 * spreading_factor + 28.0 + (16.0 * is_uplink)
        - 20.0 * header_disabled)
        / (4.0 * (spreading_factor - 2.0 * data_rate_optimization_enabled)))
        .ceil()) as u32
        * (coding_rate + 4))
        .max(0)
        + 8
}

/// T_payload as described in chapter 4 "Semtech AN1200.13 LoRa Modem Designer's Guide"
fn payload_duration(payload_symbols: u32, symbol_duration: f64) -> f64 {
    f64::from(payload_symbols) * symbol_duration
}

/// T_packet as described in chapter 4 "Semtech AN1200.13 LoRa Modem Designer's Guide"
fn packet_duration(preamble_duration: f64, payload_duration: f64) -> f64 {
    preamble_duration + payload_duration
}

/// Return airtime in ms.
fn calculate_lora_airtime(
    phy_payload_len_bytes: u32,
    spreading_factor: SpreadingFactor,
    bandwidth: Bandwidth,
    header_disabled: bool,
    is_uplink: bool,
) -> f64 {
    let t_sym = symbol_duration(spreading_factor, bandwidth);
    let preamble_duration = preamble_duration(
        LORA_PREAMBLE_LENGTH_EU868_870_IN_SYMBOLS,
        LORA_SYNC_WORD_LENGTH_IN_SYMBOLS,
        t_sym,
    );
    let payload_symbols = payload_symbols(
        phy_payload_len_bytes,
        spreading_factor,
        header_disabled,
        data_rate_optimization(&bandwidth, &spreading_factor),
        CodingRate::Cr45,
        is_uplink,
    );
    let payload_duration = payload_duration(payload_symbols, t_sym);
    (packet_duration(preamble_duration, payload_duration) * 10.0).round() / 10.0
}

/// Lookup whether or not the Low Data Rate Optimizer is used.
/// As described in chapter 4.1.2 "LoRaWAN® Regional Parameters RP002-1.0.4".
fn data_rate_optimization(bandwidth: &Bandwidth, spreading_factor: &SpreadingFactor) -> bool {
    match (bandwidth, spreading_factor) {
        (Bandwidth::Bw125, SpreadingFactor::SF7) => false,
        (Bandwidth::Bw125, SpreadingFactor::SF8) => false,
        (Bandwidth::Bw125, SpreadingFactor::SF9) => false,
        (Bandwidth::Bw125, SpreadingFactor::SF10) => false,
        (Bandwidth::Bw125, SpreadingFactor::SF11) => true,
        (Bandwidth::Bw125, SpreadingFactor::SF12) => true,
        (Bandwidth::Bw250, SpreadingFactor::SF7) => false,
        (Bandwidth::Bw250, SpreadingFactor::SF8) => false,
        (Bandwidth::Bw250, SpreadingFactor::SF9) => false,
        (Bandwidth::Bw250, SpreadingFactor::SF10) => false,
        (Bandwidth::Bw250, SpreadingFactor::SF11) => false,
        (Bandwidth::Bw250, SpreadingFactor::SF12) => true,
    }
}

/// Returns whether the modulation info belongs to an uplink or not.
/// Decision is made based on the polarization inversion.
/// Not-inverted -> uplink
/// Inverted -> downlink
/// As described in chapter 4.1.2 "LoRaWAN® Regional Parameters RP002-1.0.4".
fn is_uplink(modulation_info: &LoraModulationInfo) -> bool {
    !modulation_info.polarization_inversion
}

pub fn calc_max_downlink_airtime(
    downlink: chirpstack_api::gw::DownlinkFrame,
) -> Result<(u32, f64), AirtimeCalculationError> {
    if downlink.items.is_empty() {
        return Err(AirtimeCalculationError::NoItems);
    }
    let mut airtimes: Vec<(u32, f64)> = Vec::new();
    for item in downlink.items {
        let payload_len = u32::try_from(item.phy_payload.len())?;
        let (freq, modulation_info) =
            extract_modulation_freq_info_from_downlink_tx_info(item.tx_info)?;
        let bandwidth = Bandwidth::try_from_hz(modulation_info.bandwidth)?;
        let spreading_factor = SpreadingFactor::try_from(modulation_info.spreading_factor)?;
        airtimes.push((
            freq,
            calculate_lora_airtime(
                payload_len,
                spreading_factor,
                bandwidth,
                false,
                is_uplink(&modulation_info),
            ),
        ));
    }
    airtimes
        .sort_unstable_by(|(_, a), (_, b)| a.partial_cmp(b).expect("Encountered NaN when sorting"));
    Ok(*airtimes
        .last()
        .expect("Empty airtimes vector, cannot happen, at least one item is processed"))
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::duty_cycle_manager::calc_max_downlink_airtime;
    use chirpstack_api::gw::modulation::Parameters;
    use chirpstack_api::gw::{
        CodeRate, DownlinkFrameItem, DownlinkTxInfo, LoraModulationInfo, Modulation,
    };
    // Airtime compared to values from
    // https://www.thethingsnetwork.org/airtime-calculator/
    // https://avbentem.github.io/airtime-calculator/ttn/eu868

    #[test]
    fn calc_airtime() {
        let payload = vec![0xFF; 20];
        let mut modulation = LoraModulationInfo {
            bandwidth: 125000,
            spreading_factor: 7,
            code_rate_legacy: "".to_string(),
            polarization_inversion: false,
            ..LoraModulationInfo::default()
        };
        modulation.set_code_rate(CodeRate::Cr45);

        let downlink_frame = chirpstack_api::gw::DownlinkFrame {
            downlink_id: 0,
            downlink_id_legacy: vec![],
            items: vec![DownlinkFrameItem {
                phy_payload: payload,
                tx_info_legacy: None,
                tx_info: Some(DownlinkTxInfo {
                    frequency: 868300000,
                    power: 14,
                    modulation: Some(Modulation {
                        parameters: Some(Parameters::Lora(modulation)),
                    }),
                    ..DownlinkTxInfo::default()
                }),
            }],
            gateway_id_legacy: vec![],
            gateway_id: "abc".to_string(),
        };
        let (freq, airtime) = calc_max_downlink_airtime(downlink_frame).unwrap();
        assert_eq!(freq, 868300000);
        assert_eq!(airtime, 56.6);
    }
}
