//! Methods and enums to interact with the database.

use crate::error::DbError;
use crate::AppState;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::trace;

/// The key to retrieve data from the database.
#[derive(sqlx::Type, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(i32)]
pub enum DataKey {
    /// Configuration
    Configuration = 1,
    /// Relay messages
    RelayMessages = 2,
    /// Message buffers
    MessageBuffers = 3,
    /// Duty cycle data
    DutyCycleData = 4,
    /// Packet cache data
    PacketCacheData = 5,
}

/// Inserts data into the database.
///
/// # Error
///
/// Returns an error if:
/// - the database insert returns an error.
/// - the provided data cannot be serialized.
pub async fn insert_into_db(
    data_key: DataKey,
    data: &impl Serialize,
    db_pool: SqlitePool,
) -> Result<(), DbError> {
    trace!("Serializing data for database");
    let data_string = serde_json::to_string(data)?;
    trace!("Inserting {data_key:?} into database");
    sqlx::query!("REPLACE INTO DataTable VALUES(?,?)", data_key, data_string)
        .execute(&db_pool)
        .await?;

    Ok(())
}

/// Fetches the data form the database.
///
/// # Error
///
/// Returns an error if:
/// - the database query returns an error.
/// - the returned data cannot be deserialized.
pub async fn fetch_from_db<T: DeserializeOwned>(
    data_key: DataKey,
    db_pool: SqlitePool,
) -> Result<T, DbError> {
    trace!("Fetching data from database");
    let config_string = sqlx::query!("SELECT Data FROM DataTable WHERE DataKey=?", data_key)
        .fetch_one(&db_pool)
        .await?;

    trace!("Deserializing data from database");
    Ok(serde_json::from_str(&config_string.Data)?)
}

/// Saves the next configuration and message/packet queues to the database.
pub async fn save_state_to_db(state: Arc<AppState>) {
    trace!("Writing config to database");
    if let Err(err) = insert_into_db(
        DataKey::Configuration,
        &state.configuration.lock().await.next_configuration,
        state.db_pool.clone(),
    )
    .await
    {
        trace!("Error writing config to database: {err}");
    }

    trace!("Writing relay messages to database");
    if let Err(err) = insert_into_db(
        DataKey::RelayMessages,
        &(*state.queue_manager.relay_packet_queue.lock().await),
        state.db_pool.clone(),
    )
    .await
    {
        trace!("Error writing relay messages to database: {err}");
    }

    trace!("Writing message buffers to database");
    if let Err(err) = insert_into_db(
        DataKey::MessageBuffers,
        &(*state.queue_manager.bundle_send_buffer_queue.lock().await),
        state.db_pool.clone(),
    )
    .await
    {
        trace!("Error writing message buffers to database: {err}");
    }

    trace!("Writing duty cycle data to database");
    if let Err(err) = insert_into_db(
        DataKey::DutyCycleData,
        &state.duty_cycle_manager.lock().await.stats(),
        state.db_pool.clone(),
    )
    .await
    {
        trace!("Error writing duty cycle data to database: {err}");
    }

    trace!("Writing packet cache data to database");
    if let Err(err) = insert_into_db(
        DataKey::PacketCacheData,
        &state.packet_cache.contents().await,
        state.db_pool.clone(),
    )
    .await
    {
        trace!("Error writing packet cache data to database: {err}");
    }
}
