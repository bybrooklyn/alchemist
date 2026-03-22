CREATE TABLE IF NOT EXISTS library_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    preset TEXT NOT NULL DEFAULT 'balanced',
    codec TEXT NOT NULL DEFAULT 'av1',
    quality_profile TEXT NOT NULL DEFAULT 'balanced',
    hdr_mode TEXT NOT NULL DEFAULT 'preserve',
    audio_mode TEXT NOT NULL DEFAULT 'copy',
    crf_override INTEGER,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

ALTER TABLE watch_dirs ADD COLUMN profile_id INTEGER REFERENCES library_profiles(id);

ALTER TABLE encode_stats ADD COLUMN output_codec TEXT;

INSERT OR IGNORE INTO library_profiles
    (id, name, preset, codec, quality_profile, hdr_mode, audio_mode, notes)
VALUES
    (1, 'Space Saver', 'space_saver', 'av1', 'speed', 'tonemap', 'aac', 'Optimized for aggressive size reduction.'),
    (2, 'Quality First', 'quality_first', 'hevc', 'quality', 'preserve', 'copy', 'Prioritizes fidelity over maximum compression.'),
    (3, 'Balanced', 'balanced', 'av1', 'balanced', 'preserve', 'copy', 'Balanced compression and playback quality.'),
    (4, 'Streaming', 'streaming', 'h264', 'balanced', 'tonemap', 'aac_stereo', 'Maximizes compatibility for streaming clients.');

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '4'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
