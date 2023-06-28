-- Add migration script here
CREATE TABLE IF NOT EXISTS DataTable (
    DataKey INT NOT NULL UNIQUE,
    Data TEXT NOT NULL
);
