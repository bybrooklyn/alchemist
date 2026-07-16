# WhyTho Common Codec & Container Spec Links

Generated: 2026-06-22

This is the trimmed, practical spec/reference list for **WhyTho**: the most common video codecs, audio codecs, and media containers worth supporting first.

This is **not** every codec ever made. It intentionally focuses on formats that show up in normal media libraries, Plex/Jellyfin setups, Blu-ray/DVD rips, web video, phone recordings, and common archival/audio collections.

---

## Recommended WhyTho v1 Target

### Video decode/import

- H.264 / AVC
- H.265 / HEVC
- AV1
- VP9
- MPEG-2 Video
- MPEG-4 Part 2
- VC-1 / WMV9

### Video encode/export

- AV1
- H.265 / HEVC
- H.264 / AVC

### Audio decode/import

- AAC
- AC-3 / Dolby Digital
- E-AC-3 / Dolby Digital Plus
- DTS
- Opus
- MP3
- FLAC
- Vorbis
- ALAC

### Audio encode/export

- Opus
- AAC
- FLAC

### Containers

- MP4 / M4V / M4A
- MKV / Matroska
- WebM
- MPEG-TS / M2TS
- MOV / QuickTime
- WAV
- FLAC native
- Ogg
- AVI
- ASF / WMV / WMA

---

# Video Codecs

## 1. H.264 / AVC

**Why it matters:**  
Still the single most important compatibility codec. It is everywhere: phones, cameras, streaming, Blu-ray, downloads, Discord, browsers, and old media libraries.

**Primary specs / references:**

- ITU-T H.264 recommendation:  
  https://www.itu.int/rec/T-REC-H.264-202606-I
- ISO/IEC 14496-10, Advanced Video Coding:  
  https://www.iso.org/standard/84388.html

**WhyTho priority:**  
Must support decode/import. Still useful as an output target for maximum compatibility.

---

## 2. H.265 / HEVC

**Why it matters:**  
Very common in 4K, HDR, phone recordings, Blu-ray UHD, and modern libraries. Excellent compression, but licensing is historically annoying.

**Primary specs / references:**

- ITU-T H.265 recommendation:  
  https://www.itu.int/ITU-T/recommendations/rec.aspx?id=16661
- ISO/IEC 23008-2, High Efficiency Video Coding:  
  https://www.iso.org/standard/83639.html

**WhyTho priority:**  
Must support decode/import. Good output option when AV1 is too slow or compatibility matters.

---

## 3. AV1

**Why it matters:**  
The most important modern open video codec for WhyTho. Great for storage savings and modern web/device support.

**Primary specs / references:**

- AOMedia AV1 spec landing page:  
  https://aomedia.org/specifications/av1/
- AV1 Bitstream and Decoding Process Specification:  
  https://aomediacodec.github.io/av1-spec/
- AV1 ISO Base Media File Format Binding:  
  https://aomediacodec.github.io/av1-isobmff/

**WhyTho priority:**  
Must support decode/import and should be a main encode/export target.

---

## 4. VP9

**Why it matters:**  
Common in WebM, YouTube-era downloads, browser video, and older open web video libraries.

**Primary specs / references:**

- WebM VP9 page:  
  https://www.webmproject.org/vp9/
- VP9 Bitstream and Decoding Process Specification:  
  https://storage.googleapis.com/downloads.webmproject.org/docs/vp9/vp9-bitstream-specification-v0.6-20160331-draft.pdf
- VP Codec ISO BMFF Binding:  
  https://www.webmproject.org/vp9/mp4/

**WhyTho priority:**  
Important decode/import codec. Encoding VP9 is less important than AV1.

---

## 5. MPEG-2 Video / H.262

**Why it matters:**  
DVDs, broadcast captures, old TV recordings, some Blu-rays, MPEG-TS files, and legacy media collections.

**Primary specs / references:**

- ISO/IEC 13818-2, MPEG-2 Video:  
  https://www.iso.org/standard/61152.html
- ITU-T H.262 recommendation:  
  https://www.itu.int/rec/T-REC-H.262

**WhyTho priority:**  
Important decode/import codec. Do not prioritize encoding unless needed for DVD/broadcast compatibility.

---

## 6. MPEG-4 Part 2

**Why it matters:**  
Old DivX/Xvid-era AVI files. Not modern, but extremely common in older downloaded/ripped libraries.

**Primary specs / references:**

- Library of Congress MPEG-4 Visual / ISO/IEC 14496-2 reference:  
  https://www.loc.gov/preservation/digital/formats/fdd/fdd000080.shtml
- MPEG-4 standards overview:  
  https://www.mpeg.org/standards/MPEG-4/

**WhyTho priority:**  
Decode/import compatibility. Encoding is not worth it.

---

## 7. VC-1 / WMV9

**Why it matters:**  
Shows up in old WMV files and some Blu-ray-era content. Annoying, but real.

**Primary specs / references:**

- Library of Congress VC-1 reference:  
  https://www.loc.gov/preservation/digital/formats/fdd/fdd000095.shtml
- RFC 4425, RTP Payload Format for VC-1:  
  https://datatracker.ietf.org/doc/html/rfc4425

**WhyTho priority:**  
Decode/import compatibility only.

---

# Audio Codecs

## 1. AAC

**Why it matters:**  
The most common modern lossy audio codec. Used in MP4/M4A, phones, streaming, YouTube, Apple ecosystem, and tons of normal media files.

**Primary specs / references:**

- MPEG-4 Audio / ISO/IEC 14496-3:  
  https://www.mpeg.org/standards/MPEG-4/3/
- ISO/IEC 13818-7, MPEG-2 AAC:  
  https://www.iso.org/standard/43345.html

**WhyTho priority:**  
Must decode/import. Good compatibility output target.

---

## 2. AC-3 / Dolby Digital

**Why it matters:**  
Extremely common in DVDs, Blu-rays, TV, MKV files, and surround-sound media libraries.

**Primary specs / references:**

- Dolby standards reference for AC-3 / E-AC-3:  
  https://ott.dolby.com/OnDelKits/DDP/Dolby_Digital_Plus_Online_Delivery_Kit_v1.5/Documentation/Content_Creation/SDM/help_files/topics/intro_r_standards.html
- ETSI TS 102 366 search page:  
  https://www.etsi.org/deliver/etsi_ts/102300_102399/102366/

**WhyTho priority:**  
Must decode/import. Passthrough should be supported. Encoding can come later.

---

## 3. E-AC-3 / Dolby Digital Plus

**Why it matters:**  
Common in streaming services, modern TV files, Dolby Atmos lossy tracks, and newer media libraries.

**Primary specs / references:**

- Dolby standards reference for AC-3 / E-AC-3:  
  https://ott.dolby.com/OnDelKits/DDP/Dolby_Digital_Plus_Online_Delivery_Kit_v1.5/Documentation/Content_Creation/SDM/help_files/topics/intro_r_standards.html
- ETSI TS 102 366 search page:  
  https://www.etsi.org/deliver/etsi_ts/102300_102399/102366/

**WhyTho priority:**  
Must decode/import. Passthrough should be supported.

---

## 4. Opus

**Why it matters:**  
Best modern general-purpose open audio codec. Great for web, speech, music, low latency, and efficient transcodes.

**Primary specs / references:**

- Opus documentation page:  
  https://opus-codec.org/docs/
- RFC 6716, Opus codec:  
  https://datatracker.ietf.org/doc/html/rfc6716
- RFC 7845, Ogg Opus:  
  https://datatracker.ietf.org/doc/html/rfc7845
- RFC 7587, RTP Payload Format for Opus:  
  https://datatracker.ietf.org/doc/html/rfc7587
- RFC 8251, Opus updates:  
  https://datatracker.ietf.org/doc/html/rfc8251

**WhyTho priority:**  
Must decode/import and should be a main encode/export target.

---

## 5. MP3

**Why it matters:**  
Old as hell, but still everywhere. Music libraries, podcasts, random audio files, ancient downloads.

**Primary specs / references:**

- Library of Congress MP3 reference:  
  https://www.loc.gov/preservation/digital/formats/fdd/fdd000012.shtml
- ISO/IEC 11172-3, MPEG-1 Audio:  
  https://www.iso.org/standard/22412.html
- ISO/IEC 13818-3, MPEG-2 Audio:  
  https://www.iso.org/standard/26797.html

**WhyTho priority:**  
Must decode/import. Encoding is optional and lower priority than Opus/AAC.

---

## 6. FLAC

**Why it matters:**  
The most important lossless music/archive codec. Very common in music collections.

**Primary specs / references:**

- RFC 9639, FLAC format:  
  https://datatracker.ietf.org/doc/rfc9639/
- Xiph FLAC format page:  
  https://xiph.org/flac/format.html

**WhyTho priority:**  
Must decode/import and should encode/export for lossless archival output.

---

## 7. DTS

**Why it matters:**  
Common in Blu-ray and MKV surround-sound libraries. Includes core DTS and related DTS-HD variants in real-world files.

**Primary specs / references:**

- ETSI TS 102 114, DTS Coherent Acoustics:  
  https://www.etsi.org/deliver/etsi_ts/102100_102199/102114/01.06.01_60/ts_102114v010601p.pdf

**WhyTho priority:**  
Decode/import and passthrough. Encoding is not a v1 priority.

---

## 8. Vorbis

**Why it matters:**  
Older open audio codec. Common in Ogg files, older WebM, game assets, and older Linux/open-source media.

**Primary specs / references:**

- Xiph Vorbis docs:  
  https://xiph.org/vorbis/doc/
- Vorbis I specification:  
  https://xiph.org/vorbis/doc/Vorbis_I_spec.html

**WhyTho priority:**  
Decode/import compatibility. Encoding is less important than Opus.

---

## 9. ALAC

**Why it matters:**  
Apple Lossless. Common in Apple Music/iTunes-style lossless libraries and M4A files.

**Primary specs / references:**

- Apple Lossless Audio Codec open-source reference implementation:  
  https://github.com/macosforge/alac
- Apple Music lossless / ALAC support reference:  
  https://support.apple.com/en-us/118295

**WhyTho priority:**  
Decode/import compatibility. Encoding is optional.

---

# Containers

## 1. MP4 / M4V / M4A

**Why it matters:**  
The most important general-purpose consumer container. Used by phones, streaming, downloads, cameras, and Apple-style audio files.

**Primary specs / references:**

- ISO/IEC 14496-14, MP4 file format:  
  https://www.iso.org/standard/79110.html
- ISO Base Media File Format / ISO/IEC 14496-12:  
  https://www.iso.org/standard/83102.html
- MP4 Registration Authority:  
  https://mp4ra.org/

**WhyTho priority:**  
Must support.

---

## 2. MKV / Matroska

**Why it matters:**  
The king of media-library containers. Extremely common for movies, TV, Blu-ray remuxes, anime, subtitles, chapters, multiple audio tracks, and attachments.

**Primary specs / references:**

- RFC 9559, Matroska:  
  https://datatracker.ietf.org/doc/rfc9559/
- RFC 8794, EBML:  
  https://datatracker.ietf.org/doc/rfc8794/
- Matroska codec mappings draft:  
  https://datatracker.ietf.org/doc/draft-ietf-cellar-codec/

**WhyTho priority:**  
Must support.

---

## 3. WebM

**Why it matters:**  
Common browser/web container based on Matroska constraints. Usually VP9/AV1 + Opus/Vorbis.

**Primary specs / references:**

- WebM container guidelines:  
  https://www.webmproject.org/docs/container/
- WebM byte stream format for MSE:  
  https://www.w3.org/TR/mse-byte-stream-format-webm/

**WhyTho priority:**  
Must support if targeting web/open formats.

---

## 4. MPEG-TS / M2TS

**Why it matters:**  
Broadcast, IPTV, HLS segments, Blu-ray transport streams, TV recordings, `.ts`, `.m2ts`.

**Primary specs / references:**

- ITU-T H.222.0 / ISO/IEC 13818-1:  
  https://www.itu.int/itu-t/recommendations/rec.aspx?lang=en&rec=16266
- ISO/IEC 13818-1:  
  https://www.iso.org/standard/87603.html

**WhyTho priority:**  
Must support for real media-library compatibility.

---

## 5. MOV / QuickTime

**Why it matters:**  
Common from iPhones, cameras, editing software, and Apple workflows. Related to MP4 but not identical in all the ways that will piss you off.

**Primary specs / references:**

- Apple QuickTime File Format docs:  
  https://developer.apple.com/documentation/quicktime-file-format
- Classic QuickTime File Format PDF:  
  https://developer.apple.com/standards/qtff-2001.pdf

**WhyTho priority:**  
Important import support.

---

## 6. AVI

**Why it matters:**  
Old container, but tons of ancient DivX/Xvid/MJPEG/MP3/AC-3 files use it.

**Primary specs / references:**

- Microsoft AVI RIFF File Reference:  
  https://learn.microsoft.com/en-us/windows/win32/directshow/avi-riff-file-reference
- Library of Congress RIFF reference:  
  https://www.loc.gov/preservation/digital/formats/fdd/fdd000025.shtml

**WhyTho priority:**  
Decode/import compatibility. Do not prioritize as an output container.

---

## 7. WAV / RIFF WAVE

**Why it matters:**  
The basic uncompressed/lossless audio container. Common for PCM, samples, editing, archival work, and weird old audio.

**Primary specs / references:**

- WAVE file format documentation:  
  https://www.mmsp.ece.mcgill.ca/Documents/AudioFormats/WAVE/WAVE.html
- Microsoft/IBM Multimedia Programming Interface and Data Specifications:  
  https://www.mmsp.ece.mcgill.ca/Documents/AudioFormats/WAVE/Docs/riffmci.pdf

**WhyTho priority:**  
Must support for audio workflows.

---

## 8. FLAC Native Container

**Why it matters:**  
Standard `.flac` files in music libraries.

**Primary specs / references:**

- RFC 9639, FLAC format:  
  https://datatracker.ietf.org/doc/rfc9639/
- Xiph FLAC format page:  
  https://xiph.org/flac/format.html

**WhyTho priority:**  
Must support for music/library use.

---

## 9. Ogg

**Why it matters:**  
Common with Vorbis, Opus, Theora, and older open media files.

**Primary specs / references:**

- Xiph Ogg documentation:  
  https://xiph.org/ogg/
- RFC 3533, Ogg bitstream format:  
  https://datatracker.ietf.org/doc/html/rfc3533
- RFC 5334, Ogg media types:  
  https://datatracker.ietf.org/doc/html/rfc5334

**WhyTho priority:**  
Important import/export for Opus/Vorbis workflows.

---

## 10. ASF / WMV / WMA

**Why it matters:**  
Windows Media legacy hell. Still appears in old libraries and archives.

**Primary specs / references:**

- Microsoft ASF format overview:  
  https://learn.microsoft.com/en-us/windows/win32/wmformat/overview-of-the-asf-format
- Microsoft ASF file structure:  
  https://learn.microsoft.com/en-us/windows/win32/wmformat/asf-file-structure
- Microsoft ASF specification download:  
  https://www.microsoft.com/en-us/download/details.aspx?id=14995

**WhyTho priority:**  
Decode/import compatibility only.

---

# Common Real-World Container Matrix

| Container | Common video codecs | Common audio codecs |
|---|---|---|
| MP4 / M4V | H.264, HEVC, AV1 | AAC, AC-3, E-AC-3, ALAC |
| MKV | H.264, HEVC, AV1, VP9, MPEG-2, VC-1 | AAC, AC-3, E-AC-3, DTS, FLAC, Opus, Vorbis |
| WebM | VP9, AV1, VP8 | Opus, Vorbis |
| MPEG-TS / M2TS | H.264, HEVC, MPEG-2, VC-1 | AC-3, E-AC-3, DTS, AAC |
| MOV | H.264, HEVC, ProRes | AAC, PCM, ALAC |
| AVI | MPEG-4 Part 2, MJPEG, old H.264 | MP3, AC-3, PCM |
| WAV | none | PCM, ADPCM |
| FLAC | none | FLAC |
| Ogg | Theora, sometimes none | Vorbis, Opus, FLAC |
| ASF / WMV / WMA | WMV, VC-1 | WMA, sometimes MP3 |

---

# Suggested Implementation Order

## Phase 1: probe/detect/read

Prioritize **format detection, metadata extraction, stream listing, timestamps, durations, chapters, subtitles, and attachments**.

Implement/import support for:

1. MP4 / M4V / M4A
2. MKV / Matroska
3. WebM
4. MPEG-TS / M2TS
5. MOV
6. WAV
7. FLAC
8. Ogg
9. AVI
10. ASF / WMV / WMA

## Phase 2: decode/import

Video:

1. H.264
2. HEVC
3. AV1
4. VP9
5. MPEG-2
6. MPEG-4 Part 2
7. VC-1

Audio:

1. AAC
2. AC-3
3. E-AC-3
4. Opus
5. MP3
6. FLAC
7. DTS
8. Vorbis
9. ALAC

## Phase 3: encode/export

Video:

1. AV1
2. HEVC
3. H.264

Audio:

1. Opus
2. AAC
3. FLAC

Containers:

1. MKV
2. MP4
3. WebM
4. Ogg
5. FLAC
6. WAV

## Phase 4: passthrough/remux

This is extremely important.

Support passthrough/remux for:

- H.264
- HEVC
- AV1
- VP9
- AAC
- AC-3
- E-AC-3
- DTS
- FLAC
- Opus
- Vorbis
- ALAC

Why: sometimes the correct answer is **do not transcode the stream**. Just copy it cleanly into the output container.

---

# Notes for WhyTho

## Do not hand-roll every codec immediately

For v1, codec correctness matters more than purity. The sane path is:

1. Build robust probing/demux/remux logic.
2. Use mature codec implementations where possible.
3. Wrap them cleanly behind WhyTho-native abstractions.
4. Replace pieces with Rust-native codecs over time.

## Metadata is not optional

A transcoder that loses metadata is annoying as hell. Preserve:

- title
- language tags
- default/forced flags
- chapters
- subtitles
- fonts/attachments in MKV
- HDR metadata
- color primaries
- transfer characteristics
- matrix coefficients
- sample aspect ratio
- display aspect ratio
- channel layout
- bit depth
- sample rate
- encoder strings when useful

## Timestamp handling is where media tools go to die

Pay special attention to:

- PTS
- DTS
- time bases
- edit lists
- B-frames
- negative timestamps
- variable frame rate
- audio delay
- subtitle timing
- interleaving
- broken files

This is the stuff that makes a transcoder feel professional instead of janky.

---

# Short Final List

If WhyTho only remembers one list, make it this:

## Video

- H.264
- HEVC
- AV1
- VP9
- MPEG-2
- MPEG-4 Part 2
- VC-1

## Audio

- AAC
- AC-3
- E-AC-3
- Opus
- MP3
- FLAC
- DTS
- Vorbis
- ALAC

## Containers

- MP4 / M4V / M4A
- MKV / Matroska
- WebM
- MPEG-TS / M2TS
- MOV
- WAV
- FLAC
- Ogg
- AVI
- ASF / WMV / WMA
