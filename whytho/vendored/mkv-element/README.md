[![crates.io](https://img.shields.io/crates/v/mkv-element)](https://crates.io/crates/mkv-element)
[![docs.rs](https://img.shields.io/docsrs/mkv-element)](https://docs.rs/mkv-element)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

# mkv-element
A Rust library for reading and writing Matroska(MKV)/WebM elements.

This library provides a simple and efficient way to parse and serialize MKV elements both in memory and on disk, with support for both blocking and asynchronous I/O operations.

First and foremost, the library provides [`Header`](crate::prelude::Header) struct to read and write the element header (ID and size), and all MKV elements defined in the [Matroska specifications] as Rust structs with typed fields. All elements implement the [`Element`](crate::prelude::Element) trait, which provides methods for reading and writing the element body and [EBML ID](crate::prelude::Element::ID) for identifying the element type. As a convenience, a [`prelude`](crate::prelude) module is provided to bring all the types into scope.

To read an element, you can either use the element's [`read_from()`] method to read the entire element (header + body) from a type implementing [`std::io::Read`], or read the header first using `Header::read_from()` followed by `Element::read_element` to read the body. The latter is useful when you don't know the element type in advance. To write an element, you can use the element's [`write_to()`] method to write the entire element (header + body) to a type implementing [`std::io::Write`].

Asynchronous I/O is supported with the `tokio` feature enabled. The [`async_read_from()`], [`async_read_element()`], and [`async_write_to()`] methods are to work with types implementing [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`] respectively.

All non-master elements in this crate implements the `Deref` trait, allowing easy access to the inner value. For example, if you have an `UnsignedInteger` element, you can access its value directly using the `*` operator or by calling `.deref()`.


# Primer on Matroska/WebM (EBML) Structure
EBML([Extensible Binary Meta Language]) is a binary format similar to XML, but more efficient and flexible. It is used as the underlying format for Matroska(MKV)/WebM files. Matroska(MKV)/WebM files start with an EBML header, followed by one or more segments containing the actual media data and metadata.
Roughly, the structure looks like:

``` text
┌────────────────── MKV Structure ─────────────────┐
│ ┌────────────── EBML ──────────────┐             │
│ │ Header (Version, ReadVersion)    │             │
│ └──────────────────────────────────┘             │
│ ┌────────────── Segment(s) ────────┐             │
│ │ ┌──────────── Info ──────────┐   │             │
│ │ │ Metadata (Duration, Title) │   │             │
│ │ └────────────────────────────┘   │             │
│ │ ┌──────────── Tracks ────────┐   │             │
│ │ │ Audio/Video Tracks         │   │             │
│ │ └────────────────────────────┘   │             │
│ │ ┌──────────── SeekHead ──────┐   │             │
│ │ │ Index for Seeking          │   │             │
│ │ └────────────────────────────┘   │             │
│ │ ┌──────────── Cluster(s) ────┐   │             │
│ │ │ Media Data (Frames)        │   │             │
│ │ └────────────────────────────┘   │             │
│ │ ┌──────────── Others ────────┐   │             │
│ │ │ Cues, Chapters, Tags...    │   │             │
│ │ └────────────────────────────┘   │             │
│ └──────────────────────────────────┘             │
└──────────────────────────────────────────────────┘
```

MKV files are made of elements, each with an ID, size, and body. Elements can be of two types:
- Master elements: containers for other elements (like folders)
- Leaf elements: contain a single value of a specific type:
    - Unsigned integers
    - Signed integers
    - Floating point numbers
    - Strings (UTF-8/ASCII)
    - Binary data
    - Dates (timestamps in nanoseconds offset to 2001-01-01T00:00:00.000000000 UTC)

See the [Matroska specifications] for more details.


### Blocking I/O

```rust
use mkv_element::prelude::*;
use mkv_element::io::blocking_impl::*;

// Create an EBML header element
let ebml = Ebml {
    ebml_max_id_length: EbmlMaxIdLength(4),
    ebml_max_size_length: EbmlMaxSizeLength(8),
    doc_type: Some(DocType("matroska".to_string())),
    doc_type_version: Some(DocTypeVersion(4)),
    doc_type_read_version: Some(DocTypeReadVersion(2)),
    ..Default::default()
};

// Write to a buffer
let mut buffer = Vec::new();
ebml.write_to(&mut buffer).unwrap();

// Read back using read_from()
let parsed = Ebml::read_from(&mut &buffer[..]).unwrap();
assert_eq!(ebml, parsed);

// Or read header first, then body
let mut cursor = std::io::Cursor::new(&buffer);
let header = Header::read_from(&mut cursor).unwrap();
assert_eq!(header.id, Ebml::ID);
let parsed = Ebml::read_element(&header, &mut cursor).unwrap();
assert_eq!(ebml, parsed);
```



### Asynchronous I/O

With the `tokio` feature enabled:

```rust
# tokio_test::block_on(async {
use mkv_element::prelude::*;
use mkv_element::io::tokio_impl::*;

// Create an EBML header element
let ebml = Ebml {
    ebml_max_id_length: EbmlMaxIdLength(4),
    ebml_max_size_length: EbmlMaxSizeLength(8),
    doc_type: Some(DocType("matroska".to_string())),
    doc_type_version: Some(DocTypeVersion(4)),
    doc_type_read_version: Some(DocTypeReadVersion(2)),
    ..Default::default()
};

// Write to a buffer
let mut buffer = Vec::new();
ebml.async_write_to(&mut buffer).await.unwrap();

// Read back using async_read_from()
let parsed = Ebml::async_read_from(&mut &buffer[..]).await.unwrap();
assert_eq!(ebml, parsed);

// Or read header first, then body
let mut cursor = std::io::Cursor::new(&buffer);
let header = Header::async_read_from(&mut cursor).await.unwrap();
assert_eq!(header.id, Ebml::ID);
let parsed = Ebml::async_read_element(&header, &mut cursor).await.unwrap();
assert_eq!(ebml, parsed);
# });
```

### Efficient Metadata Parsing with View

The `view` module (requires `utils` feature) provides memory-efficient parsing of MKV files by loading only metadata while skipping cluster data:

```rust
use mkv_element::prelude::*;
use mkv_element::io::blocking_impl::*;
use mkv_element::view::MatroskaView;

// Create a sample MKV file in memory
let ebml = Ebml {
    ebml_max_id_length: EbmlMaxIdLength(4),
    ebml_max_size_length: EbmlMaxSizeLength(8),
    doc_type: Some(DocType("matroska".to_string())),
    doc_type_version: Some(DocTypeVersion(4)),
    doc_type_read_version: Some(DocTypeReadVersion(2)),
    ..Default::default()
};

let segment = Segment {
    crc32: None,
    void: None,
    seek_head: vec![],
    info: Info {
        timestamp_scale: TimestampScale(1000000),
        muxing_app: MuxingApp("mkv-element".to_string()),
        writing_app: WritingApp("example".to_string()),
        duration: Some(Duration(120000.0)),
        title: Some(Title("Sample Video".to_string())),
        ..Default::default()
    },
    cluster: vec![Cluster {
        timestamp: Timestamp(0),
        ..Default::default()
    }],
    tracks: Some(Tracks {
        track_entry: vec![TrackEntry {
            track_number: TrackNumber(1),
            track_uid: TrackUid(1234567890),
            track_type: TrackType(1), // Video
            codec_id: CodecId("V_VP9".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    }),
    cues: None,
    attachments: None,
    chapters: None,
    tags: vec![],
};

let mut buffer = Vec::new();
ebml.write_to(&mut buffer).unwrap();
segment.write_to(&mut buffer).unwrap();

// Parse with MatroskaView - skips cluster data for efficiency
let mut cursor = std::io::Cursor::new(&buffer);
let view = MatroskaView::new(&mut cursor).unwrap();

// Access metadata without loading cluster data
assert_eq!(view.ebml.doc_type.as_ref().unwrap().0, "matroska");
assert_eq!(view.segments.len(), 1);
assert_eq!(view.segments[0].info.title.as_ref().unwrap().0, "Sample Video");
assert_eq!(view.segments[0].tracks.as_ref().unwrap().track_entry.len(), 1);
```

## Features

This crate provides the following optional features:

- **`tokio`**: Enables asynchronous I/O support using Tokio. This adds `async_read_from()`, `async_read_element()`, and `async_write_to()` methods that work with types implementing `tokio::io::AsyncRead` and `tokio::io::AsyncWrite`.

- **`utils`**: Enables utility modules for working with Matroska files, such as the `view` module. The `view` module provides `MatroskaView` and `SegmentView` structs for efficiently parsing MKV file metadata without loading cluster data into memory.

To enable these features, add them to your `Cargo.toml`:

```toml
[dependencies]
mkv-element = { version = "0.3", features = ["tokio", "utils"] }
```

## Notes
1. if you need to work with actual MKV files, don't read a whole segment into memory at once, read only the parts you need instead. Real world MKV files can be very large.
2. According to the Matroska specifications, segments and clusters can have an "unknown" size (all size bytes set to 1). In that case, the segment/cluster extends to the end of the file or until the next segment/cluster. This needs to handle by the user. Trying to read such elements with this library will result in an [`ElementBodySizeUnknown`](crate::Error::ElementBodySizeUnknown) error.
3. This library does not attempt to recover from malformed/corrupted data. If such behavior is desired, extra logic can be added on top of this library.
4. Output of this library MAY NOT be the same as input, but should be semantically equivalent and valid. For example, output order of elements may differ from input order, as the order is not strictly enforced by the Matroska specifications.


## Acknowledgements
Some of the ideas and code snippets were inspired by the following sources, thanks to their authors:
- [mp4-atom](https://github.com/kixelated/mp4-atom) by *kixelated*
- [Network protocols, sans I/O](https://sans-io.readthedocs.io/)

#### License
<sup>
This project is licensed under the MIT License.
See the <a href="LICENSE">LICENSE</a> file for details.
</sup>



[Matroska specifications]: https://www.matroska.org/technical/specs/index.html
[`read_from()`]: crate::io::blocking_impl::ReadFrom::read_from
[`write_to()`]: crate::io::blocking_impl::WriteTo::write_to
[`async_read_from()`]: crate::io::tokio_impl::AsyncReadFrom::async_read_from
[`async_read_element()`]: crate::io::tokio_impl::AsyncReadElement::async_read_element
[`async_write_to()`]: crate::io::tokio_impl::AsyncWriteTo::async_write_to
[Extensible Binary Meta Language]: https://en.wikipedia.org/wiki/Extensible_Binary_Meta_Language
[`std::io::Read`]: std::io::Read
[`std::io::Write`]: std::io::Write
[`tokio::io::AsyncRead`]: tokio::io::AsyncRead
[`tokio::io::AsyncWrite`]: tokio::io::AsyncWrite