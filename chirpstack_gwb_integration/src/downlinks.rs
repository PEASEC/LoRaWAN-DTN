//! Downlinks, downlink items and builders.

pub mod downlink_builder;
pub mod downlink_item_builder;
pub mod predefined_parameters;

use std::marker::PhantomData;

/// A downlink to be sent. Multiple items may be specified, only one will be sent. Priority
/// is descending from first to last item.
#[derive(Debug, Clone)]
pub struct Downlink<DownlinkType> {
    gateway_id: String,
    downlink_id: u32,
    items: Vec<DownlinkItem<DownlinkType>>,
}

/// A single downlink to be sent.
#[derive(Debug, Clone)]
pub struct DownlinkItem<DownlinkType> {
    phy_payload: Vec<u8>,
    tx_info: TxInfo<DownlinkType>,
}

/// Part of a [`DownlinkItem`].
#[derive(Debug, Clone)]
struct TxInfo<DownlinkType> {
    frequency: u32,
    power: i32,
    lo_ra_modulation_info: LoRaModulationInfo,
    board: u32,
    antenna: u32,
    delay_timing_info: Option<DelayTimingInfo>,
    context: Option<Vec<u8>>,
    gps_epoch_timing_info: Option<GpsEpochTimingInfo>,
    downlink_type: PhantomData<DownlinkType>,
}

/// Part of a [`DownlinkItem`].
#[derive(Debug, Clone)]
struct LoRaModulationInfo {
    bandwidth: u32,
    spreading_factor: u32,
    code_rate: chirpstack_api::gw::CodeRate,
    polarization_inversion: bool,
}

/// Part of a [`DownlinkItem`].
#[derive(Debug, Clone)]
struct DelayTimingInfo {
    delay: std::time::Duration,
}

/// Part of a [`DownlinkItem`].
#[derive(Debug, Clone)]
struct GpsEpochTimingInfo {
    time_since_gps_epoch: std::time::Duration,
}

/// Marker struct for [`DownlinkBuilder`], [`DownlinkItemBuilder`], [`Downlink`] and [`DownlinkItem`].
#[derive(Debug, Clone)]
pub struct DelayTimingClassA;
/// Marker struct for [`DownlinkBuilder`], [`DownlinkItemBuilder`], [`Downlink`] and [`DownlinkItem`].
#[derive(Debug, Clone)]
pub struct GpsTimingClassB;
/// Marker struct for [`DownlinkBuilder`], [`DownlinkItemBuilder`], [`Downlink`] and [`DownlinkItem`].
#[derive(Debug, Clone)]
pub struct ImmediatelyClassC;

/// Populate [ModulationInfo](chirpstack_api::gw::downlink_tx_info::ModulationInfo) with data.
fn build_modulation_info(
    modulation_info: &LoRaModulationInfo,
    polarization_inversion: bool,
) -> chirpstack_api::gw::modulation::Parameters {
    let mut modulation_info_result = chirpstack_api::gw::LoraModulationInfo {
        bandwidth: modulation_info.bandwidth,
        spreading_factor: modulation_info.spreading_factor,
        polarization_inversion,
        ..Default::default()
    };
    modulation_info_result.set_code_rate(modulation_info.code_rate);
    chirpstack_api::gw::modulation::Parameters::Lora(modulation_info_result)
}

impl From<Downlink<DelayTimingClassA>> for chirpstack_api::gw::DownlinkFrame {
    fn from(downlink: Downlink<DelayTimingClassA>) -> Self {
        let items = {
            let mut vec = vec![];
            for mut item in downlink.items {
                let mut tx_info = chirpstack_api::gw::DownlinkTxInfo::default();
                tx_info.frequency = item.tx_info.frequency;
                tx_info.power = item.tx_info.power;
                tx_info.modulation = Some(chirpstack_api::gw::Modulation {
                    parameters: Some(build_modulation_info(
                        &item.tx_info.lo_ra_modulation_info,
                        item.tx_info.lo_ra_modulation_info.polarization_inversion,
                    )),
                });
                tx_info.board = item.tx_info.board;
                tx_info.antenna = item.tx_info.antenna;
                tx_info.timing = Some(chirpstack_api::gw::Timing{
                    parameters: Some(chirpstack_api::gw::timing::Parameters::Delay(
                        chirpstack_api::gw::DelayTimingInfo{
                            delay: Some(
                                item
                                    .tx_info.delay_timing_info
                                    .expect("This should never happen, delay_timing_info is checked to be Some(_) when building DownlinkItem<DelayTimingClassA>")
                                    .delay
                                    .into()
                            ),
                        }
                    )),
                });

                if item.tx_info.context.is_some() {
                    tx_info.context = item
                        .tx_info
                        .context
                        .take()
                        .expect("This should never happen, context is checked to be Some(_) when building DownlinkItem<DelayTimingClassA>");
                }

                let frame_item = chirpstack_api::gw::DownlinkFrameItem {
                    phy_payload: item.phy_payload,
                    tx_info: Some(tx_info),
                    tx_info_legacy: None,
                };
                vec.push(frame_item);
            }
            vec
        };

        chirpstack_api::gw::DownlinkFrame {
            downlink_id: downlink.downlink_id,
            gateway_id: downlink.gateway_id,
            items,
            ..Default::default()
        }
    }
}

impl From<Downlink<GpsTimingClassB>> for chirpstack_api::gw::DownlinkFrame {
    fn from(downlink: Downlink<GpsTimingClassB>) -> Self {
        let items = {
            let mut vec = vec![];
            for mut item in downlink.items {
                let mut tx_info = chirpstack_api::gw::DownlinkTxInfo::default();
                tx_info.frequency = item.tx_info.frequency;
                tx_info.power = item.tx_info.power;
                tx_info.modulation = Some(chirpstack_api::gw::Modulation {
                    parameters: Some(build_modulation_info(
                        &item.tx_info.lo_ra_modulation_info,
                        item.tx_info.lo_ra_modulation_info.polarization_inversion,
                    )),
                });
                tx_info.board = item.tx_info.board;
                tx_info.antenna = item.tx_info.antenna;

                tx_info.timing = Some(chirpstack_api::gw::Timing{
                    parameters: Some(chirpstack_api::gw::timing::Parameters::GpsEpoch(
                        chirpstack_api::gw::GpsEpochTimingInfo{
                            time_since_gps_epoch: Some(
                                                    item
                                                    .tx_info
                                                    .gps_epoch_timing_info
                                                    .expect("This should never happen, gps_epoch_timing_info is checked to be Some(_) when building DownlinkItem<GpsTimingClassB>")
                                                    .time_since_gps_epoch
                                                    .into()),
                        }
                    )),
                });

                if item.tx_info.context.is_some() {
                    tx_info.context = item
                        .tx_info
                        .context
                        .take()
                        .expect("This should never happen, context is checked to be Some(_) when building DownlinkItem<DelayTimingClassA>");
                }

                let frame_item = chirpstack_api::gw::DownlinkFrameItem {
                    phy_payload: item.phy_payload,
                    tx_info: Some(tx_info),
                    tx_info_legacy: None,
                };
                vec.push(frame_item);
            }
            vec
        };

        chirpstack_api::gw::DownlinkFrame {
            downlink_id: downlink.downlink_id,
            gateway_id: downlink.gateway_id,
            items,
            ..Default::default()
        }
    }
}

impl From<Downlink<ImmediatelyClassC>> for chirpstack_api::gw::DownlinkFrame {
    fn from(downlink: Downlink<ImmediatelyClassC>) -> Self {
        let items = {
            let mut vec = vec![];
            for mut item in downlink.items {
                let mut tx_info = chirpstack_api::gw::DownlinkTxInfo::default();
                tx_info.frequency = item.tx_info.frequency;
                tx_info.power = item.tx_info.power;
                tx_info.modulation = Some(chirpstack_api::gw::Modulation {
                    parameters: Some(build_modulation_info(
                        &item.tx_info.lo_ra_modulation_info,
                        item.tx_info.lo_ra_modulation_info.polarization_inversion,
                    )),
                });
                tx_info.board = item.tx_info.board;
                tx_info.antenna = item.tx_info.antenna;

                tx_info.timing = Some(chirpstack_api::gw::Timing {
                    parameters: Some(chirpstack_api::gw::timing::Parameters::Immediately(
                        chirpstack_api::gw::ImmediatelyTimingInfo {},
                    )),
                });

                if item.tx_info.context.is_some() {
                    tx_info.context = item
                        .tx_info
                        .context
                        .take()
                        .expect("This should never happen, context is checked to be Some(_) when building DownlinkItem<DelayTimingClassA>");
                }

                let frame_item = chirpstack_api::gw::DownlinkFrameItem {
                    phy_payload: item.phy_payload,
                    tx_info: Some(tx_info),
                    tx_info_legacy: None,
                };
                vec.push(frame_item);
            }
            vec
        };

        chirpstack_api::gw::DownlinkFrame {
            downlink_id: downlink.downlink_id,
            gateway_id: downlink.gateway_id,
            items,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::downlinks::downlink_builder::DownlinkBuilder;
    use crate::downlinks::downlink_item_builder::DownlinkItemBuilder;
    use crate::downlinks::predefined_parameters::{DataRate, Frequency};
    use crate::downlinks::{DelayTimingClassA, GpsTimingClassB, ImmediatelyClassC};
    use rand::Rng;

    #[test]
    fn test_create_class_a_downlink() {
        let gateway_id = "a840411d25244150".to_owned();
        let payload = vec![0xff; 20];
        let context = vec![0xff; 20];
        let frequency = Frequency::Freq868_1;
        let power = 14;
        let data_rate = DataRate::Eu863_870Dr0;
        let board = 0;
        let antenna = 0;
        let delay = std::time::Duration::from_secs(1);
        let downlink_id = rand::thread_rng().gen();
        let item = DownlinkItemBuilder::<DelayTimingClassA>::new()
            .phy_payload(payload.clone())
            .frequency(frequency)
            .power(power)
            .data_rate(data_rate)
            .board(board)
            .antenna(antenna)
            .delay(delay)
            .context(context.clone())
            .build()
            .expect("Failed to build downlink item");
        let downlink = DownlinkBuilder::new()
            .gateway_id(gateway_id.clone())
            .downlink_id(downlink_id)
            .add_item(item)
            .build()
            .expect("Failed to build downlink");
        let protobuf_downlink: chirpstack_api::gw::DownlinkFrame = downlink.into();
        let mut expected_protobuf_tx_info = chirpstack_api::gw::DownlinkTxInfo {
            frequency: 868100000,
            power,
            board,
            antenna,
            ..chirpstack_api::gw::DownlinkTxInfo::default()
        };
        let mut modulation_info_result = chirpstack_api::gw::LoraModulationInfo {
            bandwidth: 125000,
            spreading_factor: 12,
            polarization_inversion: false,
            ..Default::default()
        };
        modulation_info_result.set_code_rate(chirpstack_api::gw::CodeRate::Cr45);

        expected_protobuf_tx_info.modulation = Some(chirpstack_api::gw::Modulation {
            parameters: Some(chirpstack_api::gw::modulation::Parameters::Lora(
                modulation_info_result,
            )),
        });

        expected_protobuf_tx_info.timing = Some(chirpstack_api::gw::Timing {
            parameters: Some(chirpstack_api::gw::timing::Parameters::Delay(
                chirpstack_api::gw::DelayTimingInfo {
                    delay: Some(delay.into()),
                },
            )),
        });

        expected_protobuf_tx_info.context = context;

        let expected_protobuf_item = chirpstack_api::gw::DownlinkFrameItem {
            phy_payload: payload,
            tx_info: Some(expected_protobuf_tx_info),
            tx_info_legacy: None,
        };

        let expected_protobuf_downlink = chirpstack_api::gw::DownlinkFrame {
            gateway_id,
            items: vec![expected_protobuf_item],
            downlink_id,
            ..chirpstack_api::gw::DownlinkFrame::default()
        };
        assert_eq!(expected_protobuf_downlink, protobuf_downlink);
    }

    #[test]
    fn test_create_class_b_downlink() {
        let gateway_id = "a840411d25244150".to_owned();
        let payload = vec![0xff; 20];
        let context = vec![0xff; 20];
        let frequency = Frequency::Freq868_1;
        let power = 14;
        let data_rate = DataRate::Eu863_870Dr0;
        let board = 0;
        let antenna = 0;
        let time_since_gps_epoch = std::time::Duration::from_secs(1);
        let downlink_id = rand::thread_rng().gen();
        let item = DownlinkItemBuilder::<GpsTimingClassB>::new()
            .phy_payload(payload.clone())
            .frequency(frequency)
            .power(power)
            .data_rate(data_rate)
            .board(board)
            .antenna(antenna)
            .context(context.clone())
            .time_since_gps_epoch(time_since_gps_epoch)
            .build()
            .expect("Failed to build downlink item");
        let downlink = DownlinkBuilder::new()
            .gateway_id(gateway_id.clone())
            .downlink_id(downlink_id)
            .add_item(item)
            .build()
            .expect("Failed to build downlink");
        let protobuf_downlink: chirpstack_api::gw::DownlinkFrame = downlink.into();
        let mut expected_protobuf_tx_info = chirpstack_api::gw::DownlinkTxInfo {
            frequency: 868100000,
            power,
            board,
            antenna,
            ..chirpstack_api::gw::DownlinkTxInfo::default()
        };
        let mut modulation_info_result = chirpstack_api::gw::LoraModulationInfo {
            bandwidth: 125000,
            spreading_factor: 12,
            polarization_inversion: false,
            ..Default::default()
        };
        modulation_info_result.set_code_rate(chirpstack_api::gw::CodeRate::Cr45);

        expected_protobuf_tx_info.modulation = Some(chirpstack_api::gw::Modulation {
            parameters: Some(chirpstack_api::gw::modulation::Parameters::Lora(
                modulation_info_result,
            )),
        });

        expected_protobuf_tx_info.timing = Some(chirpstack_api::gw::Timing {
            parameters: Some(chirpstack_api::gw::timing::Parameters::GpsEpoch(
                chirpstack_api::gw::GpsEpochTimingInfo {
                    time_since_gps_epoch: Some(time_since_gps_epoch.into()),
                },
            )),
        });

        expected_protobuf_tx_info.context = context;

        let expected_protobuf_item = chirpstack_api::gw::DownlinkFrameItem {
            phy_payload: payload,
            tx_info: Some(expected_protobuf_tx_info),
            tx_info_legacy: None,
        };

        let expected_protobuf_downlink = chirpstack_api::gw::DownlinkFrame {
            gateway_id,
            items: vec![expected_protobuf_item],
            downlink_id,
            ..chirpstack_api::gw::DownlinkFrame::default()
        };
        assert_eq!(expected_protobuf_downlink, protobuf_downlink);
    }

    #[test]
    fn test_create_class_c_downlink() {
        let gateway_id = "a840411d25244150".to_owned();
        let payload = vec![0xff; 20];
        let context = vec![0xff; 20];
        let frequency = Frequency::Freq868_1;
        let power = 14;
        let data_rate = DataRate::Eu863_870Dr0;
        let board = 0;
        let antenna = 0;
        let downlink_id = rand::thread_rng().gen();
        let item = DownlinkItemBuilder::<ImmediatelyClassC>::new()
            .phy_payload(payload.clone())
            .frequency(frequency)
            .power(power)
            .data_rate(data_rate)
            .board(board)
            .antenna(antenna)
            .context(context.clone())
            .build()
            .expect("Failed to build downlink item");
        let downlink = DownlinkBuilder::new()
            .gateway_id(gateway_id.clone())
            .downlink_id(downlink_id)
            .add_item(item)
            .build()
            .expect("Failed to build downlink");
        let protobuf_downlink: chirpstack_api::gw::DownlinkFrame = downlink.into();
        let mut expected_protobuf_tx_info = chirpstack_api::gw::DownlinkTxInfo {
            frequency: 868100000,
            power,
            board,
            antenna,
            ..chirpstack_api::gw::DownlinkTxInfo::default()
        };
        let mut modulation_info_result = chirpstack_api::gw::LoraModulationInfo {
            bandwidth: 125000,
            spreading_factor: 12,
            polarization_inversion: false,
            ..Default::default()
        };
        modulation_info_result.set_code_rate(chirpstack_api::gw::CodeRate::Cr45);

        expected_protobuf_tx_info.modulation = Some(chirpstack_api::gw::Modulation {
            parameters: Some(chirpstack_api::gw::modulation::Parameters::Lora(
                modulation_info_result,
            )),
        });

        expected_protobuf_tx_info.timing = Some(chirpstack_api::gw::Timing {
            parameters: Some(chirpstack_api::gw::timing::Parameters::Immediately(
                chirpstack_api::gw::ImmediatelyTimingInfo {},
            )),
        });

        expected_protobuf_tx_info.context = context;

        let expected_protobuf_item = chirpstack_api::gw::DownlinkFrameItem {
            phy_payload: payload,
            tx_info: Some(expected_protobuf_tx_info),
            tx_info_legacy: None,
        };

        let expected_protobuf_downlink = chirpstack_api::gw::DownlinkFrame {
            gateway_id,
            items: vec![expected_protobuf_item],
            downlink_id,
            ..chirpstack_api::gw::DownlinkFrame::default()
        };
        assert_eq!(expected_protobuf_downlink, protobuf_downlink);
    }
}
