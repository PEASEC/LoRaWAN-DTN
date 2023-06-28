//! End device ID.

use crate::error::TryFromEndDeviceId;
use bp7::EndpointID;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// End device ID used to identify network participants.
#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash, Serialize, Deserialize, JsonSchema)]
pub struct EndDeviceId(pub u32);

impl TryFrom<EndpointID> for EndDeviceId {
    type Error = TryFromEndDeviceId;

    fn try_from(endpoint_id: EndpointID) -> Result<Self, Self::Error> {
        if let EndpointID::Dtn(_, address) = endpoint_id {
            let inner = u32::from_str(address.node_name())?;
            Ok(EndDeviceId(inner))
        } else {
            Err(TryFromEndDeviceId::NoDtnAddress)
        }
    }
}

impl TryFrom<EndDeviceId> for EndpointID {
    type Error = bp7::eid::EndpointIdError;

    fn try_from(end_device_id: EndDeviceId) -> Result<Self, Self::Error> {
        EndpointID::with_dtn(&end_device_id.0.to_string())
    }
}

/// Managed end device ID used to identify network participants. Only used for end device ids
/// registered at the Spatz instance. Keeps clear text representation of hash for ease of management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedEndDeviceId {
    /// CRC32 hash of the phone number.
    hash: u32,
    /// Phone number of the end device.
    phone_number: String,
}

impl ManagedEndDeviceId {
    /// Creates a new [`ManagedEndDevice`].
    pub fn new(number: String) -> Self {
        Self {
            hash: crc32fast::hash(number.as_bytes()),
            phone_number: number,
        }
    }

    /// Returns the hash of the [`ManagedEndDevice`].
    pub fn hash(&self) -> u32 {
        self.hash
    }

    /// Returns the phone number of the [`ManagedEndDevice`].
    pub fn phone_number(&self) -> String {
        self.phone_number.clone()
    }
}

impl From<ManagedEndDeviceId> for EndDeviceId {
    fn from(managed_end_device_id: ManagedEndDeviceId) -> Self {
        EndDeviceId(managed_end_device_id.hash)
    }
}

impl From<String> for ManagedEndDeviceId {
    fn from(number: String) -> Self {
        ManagedEndDeviceId {
            hash: crc32fast::hash(number.as_bytes()),
            phone_number: number,
        }
    }
}

impl From<&String> for ManagedEndDeviceId {
    fn from(number: &String) -> Self {
        ManagedEndDeviceId {
            hash: crc32fast::hash(number.as_bytes()),
            phone_number: number.clone(),
        }
    }
}

impl From<u32> for ManagedEndDeviceId {
    fn from(value: u32) -> Self {
        ManagedEndDeviceId {
            phone_number: String::new(),
            hash: value,
        }
    }
}

impl From<EndDeviceId> for ManagedEndDeviceId {
    fn from(value: EndDeviceId) -> Self {
        ManagedEndDeviceId {
            phone_number: String::new(),
            hash: value.0,
        }
    }
}

impl From<&EndDeviceId> for ManagedEndDeviceId {
    fn from(value: &EndDeviceId) -> Self {
        ManagedEndDeviceId {
            phone_number: String::new(),
            hash: value.0,
        }
    }
}

impl Eq for ManagedEndDeviceId {}
impl PartialEq for ManagedEndDeviceId {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Hash for ManagedEndDeviceId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}
