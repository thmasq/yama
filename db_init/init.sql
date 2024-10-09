CREATE TABLE IF NOT EXISTS image_hashes (
    id SERIAL PRIMARY KEY,
    hash VARCHAR(256) NOT NULL,
    guild_id BIGINT NOT NULL,
    channel_id BIGINT NOT NULL,
    message_id BIGINT NOT NULL,
    timestamp TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (hash, guild_id)
);
