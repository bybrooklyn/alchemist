# Stream Rules

Control which audio tracks are kept, stripped, or filtered during transcoding.

Stream rules let you control which audio tracks survive the
transcode. They run during the planning phase - before FFmpeg
is invoked - and determine which audio streams are included
in the output.

## Why use stream rules?

Blu-ray rips often contain three to eight audio tracks:
the main soundtrack in multiple formats (TrueHD, DTS-HD,
AC3), commentary tracks, descriptive audio, and foreign
dubs. Most of that is wasted space if you only ever watch
in one language. Stream rules let you strip it automatically.

## Available rules

### Strip audio by title

Removes audio tracks whose title contains any of the specified
strings. Comparison is case-insensitive.

```toml
[transcode.stream_rules]
strip_audio_by_title = ["commentary", "director", "description", "ad"]
```

This would strip tracks titled "Director's Commentary",
"Audio Description", "AD", etc.

### Keep audio by language

Keeps only audio tracks whose language tag matches the
specified ISO 639-2 codes. All other tracks are dropped.

```toml
[transcode.stream_rules]
keep_audio_languages = ["eng", "jpn"]
```

If a track has no language tag, it is kept regardless of this
setting (to avoid accidentally removing the only audio).

### Keep only default audio

Keeps only the track flagged as the default in the source
file. Useful when you trust the source to have flagged the
main soundtrack correctly.

```toml
[transcode.stream_rules]
keep_only_default_audio = true
```

> Note: `keep_only_default_audio` is evaluated after language
> rules. If no track survives the language filter, the
> default track is kept as a fallback to ensure the output
> always has audio.

## Rule evaluation order

1. `strip_audio_by_title` - runs first, removes matched tracks
2. `keep_audio_languages` - runs on the remaining tracks
3. `keep_only_default_audio` - runs last on the remaining tracks
4. Fallback: if no tracks remain, the original default track
   is kept

## Example: lean English-only output

```toml
[transcode.stream_rules]
strip_audio_by_title = ["commentary", "description", "ad"]
keep_audio_languages = ["eng"]
```

## Configuring in the UI

Stream rules are set in **Settings -> Transcoding -> Stream
Rules**. Changes take effect for jobs queued after the save.
Active jobs continue with the rules they were planned with.
