mod announcement;
mod bundle;

pub use announcement::{AnnouncementReceiveBuffer, CombinedAnnouncement};
pub use bundle::{BundleReceiveBuffer, CombinedBundle};

use bp7::DtnTime;

/// Convert a unix timestamp to a [`bp7::DtnTime`].
pub fn unix_ts_to_dtn_time(timestamp: u64) -> DtnTime {
    (timestamp - bp7::dtntime::SECONDS1970_TO2K) * 1000
}
