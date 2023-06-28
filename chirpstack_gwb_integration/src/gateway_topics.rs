//! ChirpStack MQTT topic parsing.

use crate::error::TopicParsingError;

/// LoRaWAN regions.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LoRaWanRegion {
    As923,
    As923_2,
    As923_3,
    As923_4,
    Au915,
    Cn470,
    Eu433,
    Eu868,
    In865,
    Kr920,
    Ru864,
    Us915,
    Ism2400,
}

impl TryFrom<&str> for LoRaWanRegion {
    type Error = TopicParsingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "as923" => Ok(LoRaWanRegion::As923),
            "as923-2" => Ok(LoRaWanRegion::As923_2),
            "as923-3" => Ok(LoRaWanRegion::As923_3),
            "as923-4" => Ok(LoRaWanRegion::As923_4),
            "au915" => Ok(LoRaWanRegion::Au915),
            "cn470" => Ok(LoRaWanRegion::Cn470),
            "eu433" => Ok(LoRaWanRegion::Eu433),
            "eu868" => Ok(LoRaWanRegion::Eu868),
            "in865" => Ok(LoRaWanRegion::In865),
            "kr920" => Ok(LoRaWanRegion::Kr920),
            "ru864" => Ok(LoRaWanRegion::Ru864),
            "us915" => Ok(LoRaWanRegion::Us915),
            "ism2400" => Ok(LoRaWanRegion::Ism2400),
            _ => Err(TopicParsingError::LoRaWanRegion {
                was: value.to_owned(),
            }),
        }
    }
}

/// MQTT Topic types
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TopicType {
    Event(EventType),
    State(StateType),
    Command(CommandType),
}

impl TryFrom<(&str, &str)> for TopicType {
    type Error = TopicParsingError;

    fn try_from(value: (&str, &str)) -> Result<Self, Self::Error> {
        let (topic_type, topic_sub_type) = value;
        match topic_type {
            "event" => Ok(TopicType::Event(topic_sub_type.try_into()?)),
            "state" => Ok(TopicType::State(topic_sub_type.try_into()?)),
            "command" => Ok(TopicType::Command(topic_sub_type.try_into()?)),
            _ => Err(TopicParsingError::TopicType {
                was: topic_type.to_owned(),
            }),
        }
    }
}

/// All possible event types.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EventType {
    Stats,
    Up,
    Ack,
    Exec,
    Raw,
}

impl TryFrom<&str> for EventType {
    type Error = TopicParsingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "stats" => Ok(EventType::Stats),
            "up" => Ok(EventType::Up),
            "ack" => Ok(EventType::Ack),
            "exec" => Ok(EventType::Exec),
            "raw" => Ok(EventType::Raw),

            _ => Err(TopicParsingError::EventType {
                was: value.to_owned(),
            }),
        }
    }
}

/// All possible state types.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StateType {
    Conn,
}

impl TryFrom<&str> for StateType {
    type Error = TopicParsingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "conn" => Ok(StateType::Conn),
            _ => Err(TopicParsingError::StateType {
                was: value.to_owned(),
            }),
        }
    }
}

/// All possible command types.
#[allow(missing_docs)]
#[allow(clippy::missing_docs_in_private_items)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CommandType {
    Down,
    Config,
    Exec,
    Raw,
}

impl TryFrom<&str> for CommandType {
    type Error = TopicParsingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "down" => Ok(CommandType::Down),
            "config" => Ok(CommandType::Config),
            "exec" => Ok(CommandType::Exec),
            "raw" => Ok(CommandType::Raw),
            _ => Err(TopicParsingError::CommandType {
                was: value.to_owned(),
            }),
        }
    }
}

/// Parsed topic information.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParsedTopic {
    /// The region.
    pub region: LoRaWanRegion,
    /// The gateway ID.
    pub gateway_id: String,
    /// The type of topic.
    pub topic_type: TopicType,
}

impl TryFrom<&str> for ParsedTopic {
    type Error = TopicParsingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let split_topic: Vec<&str> = value.split('/').collect();
        if split_topic.len() < 5 {
            return Err(TopicParsingError::TooShort {
                length: split_topic.len(),
            });
        }
        if split_topic.len() > 5 {
            return Err(TopicParsingError::TooLong {
                length: split_topic.len(),
            });
        }
        let region =
            LoRaWanRegion::try_from(*split_topic.first().expect("Length was checked to be 5."))?;
        if *split_topic.get(1).expect("Length was checked to be 5.") != "gateway" {
            return Err(TopicParsingError::NoGatewayMarker);
        }
        let gateway_id = (*split_topic.get(2).expect("Length was checked to be 5.")).to_owned();
        let topic_type = TopicType::try_from((
            *split_topic.get(3).expect("Length was checked to be 5."),
            *split_topic.get(4).expect("Length was checked to be 5."),
        ))?;

        Ok(Self {
            region,
            gateway_id,
            topic_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::error::TopicParsingError;
    use crate::gateway_topics::{CommandType, LoRaWanRegion, ParsedTopic, TopicType};

    #[test]
    fn parse_topic() {
        let topic = "eu868/gateway/ac1f09fffe060970/command/down";
        let parsed_topic: ParsedTopic = topic.try_into().unwrap();
        let expected_parse_topic = ParsedTopic {
            region: LoRaWanRegion::Eu868,
            gateway_id: "ac1f09fffe060970".to_string(),
            topic_type: TopicType::Command(CommandType::Down),
        };
        assert_eq!(parsed_topic, expected_parse_topic);
    }

    #[test]
    fn parse_topic_wrong_region() {
        let topic = "eu68/gateway/ac1f09fffe060970/command/down";
        let parsed_topic: Result<ParsedTopic, TopicParsingError> = topic.try_into();
        match parsed_topic.err().unwrap() {
            TopicParsingError::LoRaWanRegion { was } => {
                assert_eq!(was, "eu68".to_owned());
            }
            _ => panic!("Wrong error returned."),
        }
    }

    #[test]
    fn parse_topic_wrong_topic1() {
        let topic = "eu868/gateway/ac1f09fffe060970/comand/down";
        let parsed_topic: Result<ParsedTopic, TopicParsingError> = topic.try_into();
        match parsed_topic.err().unwrap() {
            TopicParsingError::TopicType { was } => {
                assert_eq!(was, "comand".to_owned());
            }
            _ => panic!("Wrong error returned."),
        }
    }

    #[test]
    fn parse_topic_wrong_topic2() {
        let topic = "eu868/gateway/ac1f09fffe060970/command/dow";
        let parsed_topic: Result<ParsedTopic, TopicParsingError> = topic.try_into();
        match parsed_topic.err().unwrap() {
            TopicParsingError::CommandType { was } => {
                assert_eq!(was, "dow".to_owned());
            }
            _ => panic!("Wrong error returned."),
        }
    }

    #[test]
    fn parse_topic_too_short() {
        let topic = "eu868/gateway/ac1f09fffe060970";
        let parsed_topic: Result<ParsedTopic, TopicParsingError> = topic.try_into();
        match parsed_topic.err().unwrap() {
            TopicParsingError::TooShort { length } => {
                assert_eq!(length, 3);
            }
            _ => panic!("Wrong error returned."),
        }
    }

    #[test]
    fn parse_topic_too_long() {
        let topic = "eu868/gateway/ac1f09fffe060970/command/down/test";
        let parsed_topic: Result<ParsedTopic, TopicParsingError> = topic.try_into();
        match parsed_topic.err().unwrap() {
            TopicParsingError::TooLong { length } => {
                assert_eq!(length, 6);
            }
            _ => panic!("Wrong error returned."),
        }
    }

    #[test]
    fn parse_topic_no_gateway_marker() {
        let topic = "eu868/gatway/ac1f09fffe060970/command/down";
        let parsed_topic: Result<ParsedTopic, TopicParsingError> = topic.try_into();
        match parsed_topic.err().unwrap() {
            TopicParsingError::NoGatewayMarker => {}
            _ => panic!("Wrong error returned."),
        }
    }
}
