CREATE TABLE IF NOT EXISTS file_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    delete_source BOOLEAN NOT NULL DEFAULT 0,
    output_extension TEXT NOT NULL DEFAULT 'mkv',
    output_suffix TEXT NOT NULL DEFAULT '-alchemist',
    replace_strategy TEXT NOT NULL DEFAULT 'keep'
);

-- Ensure default row exists
INSERT OR IGNORE INTO file_settings (id, delete_source, output_extension, output_suffix, replace_strategy)
VALUES (1, 0, 'mkv', '-alchemist', 'keep');
