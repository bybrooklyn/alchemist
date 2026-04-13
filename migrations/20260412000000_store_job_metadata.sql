-- Store input metadata as JSON to avoid live re-probing completed jobs
ALTER TABLE jobs ADD COLUMN input_metadata_json TEXT;
