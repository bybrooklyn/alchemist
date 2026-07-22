---
title: Stream Rules
description: Control which audio tracks survive transcoding.
---

Stream rules filter audio tracks during the planning phase,
before FFmpeg is invoked.

## Why use them

Blu-ray rips often contain three to eight audio tracks:
TrueHD, DTS-HD, AC3, commentary, descriptive audio, foreign
dubs. Stream rules strip unwanted tracks automatically.

## Available rules

### strip_audio_by_title

Removes tracks whose title contains any of the specified
strings (case-insensitive).

```toml
[transcode.stream_rules]
strip_audio_by_title = ["commentary", "director", "description", "ad"]
```

### keep_audio_languages

Keeps only tracks whose ISO 639-2 language tag matches.
Tracks with no language tag are always kept.

```toml
[transcode.stream_rules]
keep_audio_languages = ["eng", "jpn"]
```

### keep_only_default_audio

Keeps only the track flagged as default in the source.

```toml
[transcode.stream_rules]
keep_only_default_audio = true
```

## Evaluation order

1. `strip_audio_by_title`
2. `keep_audio_languages`
3. `keep_only_default_audio`
4. Fallback: if no tracks survive, the original default is kept

## Example: lean English-only output

```toml
[transcode.stream_rules]
strip_audio_by_title = ["commentary", "description", "ad"]
keep_audio_languages = ["eng"]
```

Configure in **Settings → Transcoding → Stream Rules**.
