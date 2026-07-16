# matroska-demuxer

[![Latest version](https://img.shields.io/crates/v/matroska-demuxer.svg)](https://crates.io/crates/matroska-demuxer)
[![Documentation](https://docs.rs/matroska-demuxer/badge.svg)](https://docs.rs/matroska-demuxer)
![ZLIB](https://img.shields.io/badge/license-zlib-blue.svg)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

A demuxer that can demux Matroska and WebM container files.

For simplicity only the elements supported by both Matroska and WebM are supported.

## Integration test

When built inside the WhyTho workspace, the integration tests resolve
`test1.mkv` through `test8.mkv` from the adjacent vendored
`mkv-element/matroska-test-files` suite. For a standalone checkout, download
[the Matroska test suite](https://sourceforge.net/projects/matroska/files/test_files/matroska_test_w1_1.zip/download)
and extract those files into the `tests/data` folder.

## License

Licensed under MIT or Apache-2.0 or ZLIB.
