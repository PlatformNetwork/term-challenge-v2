-- Migration 023: Subnet Settings for Owner Controls
--
-- Stores subnet-level settings like uploads_enabled, validation_enabled, paused.
-- These settings are controlled by the subnet owner via sudo operations.

-- Create subnet_settings table (singleton - only one row)
CREATE TABLE IF NOT EXISTS subnet_settings (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    uploads_enabled BOOLEAN NOT NULL DEFAULT true,
    validation_enabled BOOLEAN NOT NULL DEFAULT true,
    paused BOOLEAN NOT NULL DEFAULT false,
    owner_hotkey TEXT DEFAULT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by TEXT DEFAULT 'system'
);

-- Insert default settings
INSERT INTO subnet_settings (id, uploads_enabled, validation_enabled, paused, owner_hotkey, updated_by)
VALUES (1, true, true, false, NULL, 'system')
ON CONFLICT (id) DO NOTHING;
