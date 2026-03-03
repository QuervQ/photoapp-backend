CREATE TABLE IF NOT EXISTS assets (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    storage_path TEXT NOT NULL UNIQUE,
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT asset_kind_valid CHECK (kind IN ('image', 'worldmap'))
);

CREATE TABLE IF NOT EXISTS worldmaps (
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    version INTEGER NOT NULL,
    asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (room_id, version)
);

CREATE INDEX IF NOT EXISTS idx_worldmaps_room_id_created_at ON worldmaps(room_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_worldmaps_asset_id ON worldmaps(asset_id);

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_active ON refresh_tokens(user_id, revoked_at, expires_at);
