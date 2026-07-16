//! A View of a Matroska file, parsing w/o loading clusters into memory.

use std::mem::take;

use crate::element::Element;
use crate::master::*;

/// View of a Matroska file, parsing the EBML and Segment headers, but not loading Clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct MatroskaView {
    /// The EBML header.
    pub ebml: Ebml,
    /// The Segment views, as there can be multiple segments in a Matroska file.
    pub segments: Vec<SegmentView>,
}

impl MatroskaView {
    /// Create a new MatroskaView by parsing the EBML header and all Segment headers,
    /// but skipping Cluster data to avoid loading it into memory.
    pub fn new<R>(reader: &mut R) -> crate::Result<Self>
    where
        R: std::io::Read + std::io::Seek + ?Sized,
    {
        use crate::io::blocking_impl::*;

        // Read the EBML header
        let ebml = Ebml::read_from(reader)?;

        // Parse all segments in the file
        let segments = SegmentView::new(reader)?;

        // At least one segment is required
        if segments.is_empty() {
            return Err(crate::Error::MissingElement(Segment::ID));
        }

        Ok(MatroskaView { ebml, segments })
    }

    /// Create a new MatroskaView by parsing the EBML header and all Segment headers,
    /// but skipping Cluster data to avoid loading it into memory.
    #[cfg(feature = "tokio")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
    pub async fn new_async<R>(reader: &mut R) -> crate::Result<Self>
    where
        R: tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + ?Sized,
    {
        use crate::io::tokio_impl::*;

        // Read the EBML header
        let ebml = Ebml::async_read_from(reader).await?;

        // Parse all segments in the file
        let segments = SegmentView::new_async(reader).await?;

        Ok(MatroskaView { ebml, segments })
    }
}

/// View of a Segment, parsing the Segment header, but not loading Clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentView {
    /// Contains seeking information of Top-Level Elements; see data-layout.
    pub seek_head: Vec<SeekHead>,
    /// Contains general information about the Segment.
    pub info: Info,
    /// A Top-Level Element of information with many tracks described.
    pub tracks: Option<Tracks>,
    /// A Top-Level Element to speed seeking access. All entries are local to the Segment. This Element **SHOULD** be set when the Segment is not transmitted as a live stream (see #livestreaming).
    pub cues: Option<Cues>,
    /// Contain attached files.
    pub attachments: Option<Attachments>,
    /// A system to define basic menus and partition data. For more detailed information, look at the Chapters explanation in chapters.
    pub chapters: Option<Chapters>,
    /// Element containing metadata describing Tracks, Editions, Chapters, Attachments, or the Segment as a whole. A list of valid tags can be found in [Matroska tagging RFC](https://www.matroska.org/technical/tagging.html).
    pub tags: Vec<Tags>,
    /// The position of the Segment data (after the Segment header).
    pub segment_data_position: u64,
    /// The position of the first Cluster in the Segment. 0 if no Cluster found.
    pub first_cluster_position: u64,
}

impl SegmentView {
    /// Create a new SegmentView by parsing the Segment header and metadata elements,
    /// but skipping Cluster data to avoid loading it into memory.
    pub fn new<R>(reader: &mut R) -> crate::Result<Vec<Self>>
    where
        R: std::io::Read + std::io::Seek + ?Sized,
    {
        let mut out = vec![];

        use crate::io::blocking_impl::*;
        use std::io::SeekFrom;

        // Read the Segment header
        let segment_header = crate::base::Header::read_from(reader)?;
        if segment_header.id != Segment::ID {
            return Err(crate::Error::MissingElement(Segment::ID));
        }

        let mut segment_data_position = reader.stream_position()?;

        let mut seek_head = Vec::new();
        let mut info = None;
        let mut tracks = None;
        let mut cues = None;
        let mut attachments = None;
        let mut chapters = None;
        let mut tags = Vec::new();
        let mut first_cluster_position = 0;

        // Parse segment elements
        loop {
            use crate::base::Header;

            let current_position = reader.stream_position()?;
            let Ok(header) = Header::read_from(reader) else {
                break;
            };
            if header.id == Cluster::ID && first_cluster_position == 0 {
                first_cluster_position = current_position;
            }

            // Check if we've reached the end of the segment
            match header.id {
                SeekHead::ID => seek_head.push(SeekHead::read_element(&header, reader)?),
                Info::ID => info = Some(Info::read_element(&header, reader)?),
                Tracks::ID => tracks = Some(Tracks::read_element(&header, reader)?),
                Cues::ID => cues = Some(Cues::read_element(&header, reader)?),
                Attachments::ID => attachments = Some(Attachments::read_element(&header, reader)?),
                Chapters::ID => chapters = Some(Chapters::read_element(&header, reader)?),
                Tags::ID => tags.push(Tags::read_element(&header, reader)?),
                Cluster::ID => {
                    // try to skip, or else break
                    use crate::base::VInt64;
                    let mut seeks: Vec<(VInt64, u64)> = seek_head
                        .iter()
                        .flat_map(|sh| {
                            sh.seek.iter().flat_map(|s| {
                                let mut id = &s.seek_id[..];
                                let a = VInt64::read_from(&mut id);
                                match a {
                                    Ok(v) => Some((v, *s.seek_position + segment_data_position)),
                                    Err(e) => {
                                        log::warn!("Failed to read seek_id as VInt: {e}, skip...");
                                        None
                                    }
                                }
                            })
                        })
                        .collect();

                    seeks.sort_by_key(|a| a.1);

                    // find position larger than first_cluster_position
                    if let Some(pos) = seeks.iter().find(|(_, pos)| *pos > first_cluster_position) {
                        reader.seek(SeekFrom::Start(pos.1))?;
                        continue;
                    }

                    if segment_header.size.is_unknown {
                        break;
                    } else {
                        let eos = segment_data_position + *segment_header.size;
                        reader.seek(SeekFrom::Start(eos))?;
                        continue;
                    }
                }
                Segment::ID => {
                    out.push(SegmentView {
                        seek_head: take(&mut seek_head),
                        // Info is required in a valid Matroska file
                        info: info.take().ok_or(crate::Error::MissingElement(Info::ID))?,
                        tracks: tracks.take(),
                        cues: cues.take(),
                        attachments: attachments.take(),
                        chapters: chapters.take(),
                        tags: take(&mut tags),
                        first_cluster_position: take(&mut first_cluster_position),
                        segment_data_position: take(&mut segment_data_position),
                    });
                    segment_data_position = reader.stream_position()?;
                }
                _ => {
                    use log::warn;
                    use std::io::Read;
                    // Skip unknown elements, here we read and discard the data for efficiency
                    std::io::copy(&mut reader.take(*header.size), &mut std::io::sink())?;
                    warn!("Skipped unknown element with ID: {}", header.id);
                }
            }
        }

        // Info is required in a valid Matroska file
        let info = info.ok_or(crate::Error::MissingElement(Info::ID))?;

        out.push(SegmentView {
            seek_head,
            info,
            tracks,
            cues,
            attachments,
            chapters,
            tags,
            first_cluster_position,
            segment_data_position,
        });
        Ok(out)
    }

    /// Create a new SegmentView by parsing the Segment header and metadata elements,
    /// but skipping Cluster data to avoid loading it into memory.
    #[cfg(feature = "tokio")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
    pub async fn new_async<R>(reader: &mut R) -> crate::Result<Vec<Self>>
    where
        R: tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + ?Sized,
    {
        let mut out = vec![];

        use crate::io::tokio_impl::*;
        use tokio::io::AsyncSeekExt;

        // Read the Segment header
        let segment_header = crate::base::Header::async_read_from(reader).await?;
        if segment_header.id != Segment::ID {
            return Err(crate::Error::MissingElement(Segment::ID));
        }

        let mut segment_data_position = reader.stream_position().await?;

        let mut seek_head = Vec::new();
        let mut info = None;
        let mut tracks = None;
        let mut cues = None;
        let mut attachments = None;
        let mut chapters = None;
        let mut tags = Vec::new();
        let mut first_cluster_position = 0;

        // Parse segment elements
        loop {
            use crate::base::Header;

            let current_position = reader.stream_position().await?;
            let Ok(header) = Header::async_read_from(reader).await else {
                break;
            };
            if header.id == Cluster::ID && first_cluster_position == 0 {
                first_cluster_position = current_position;
            }

            // Check if we've reached the end of the segment
            match header.id {
                SeekHead::ID => {
                    seek_head.push(SeekHead::async_read_element(&header, reader).await?)
                }
                Info::ID => info = Some(Info::async_read_element(&header, reader).await?),
                Tracks::ID => tracks = Some(Tracks::async_read_element(&header, reader).await?),
                Cues::ID => cues = Some(Cues::async_read_element(&header, reader).await?),
                Attachments::ID => {
                    attachments = Some(Attachments::async_read_element(&header, reader).await?)
                }
                Chapters::ID => {
                    chapters = Some(Chapters::async_read_element(&header, reader).await?)
                }
                Tags::ID => tags.push(Tags::async_read_element(&header, reader).await?),
                Cluster::ID => {
                    // try to skip, or else break
                    use crate::base::VInt64;
                    let mut seeks: Vec<(VInt64, u64)> = seek_head
                        .iter()
                        .flat_map(|sh| {
                            sh.seek.iter().flat_map(|s| {
                                use crate::io::blocking_impl::ReadFrom;

                                let mut id = &s.seek_id[..];
                                let a = VInt64::read_from(&mut id);
                                match a {
                                    Ok(v) => Some((v, *s.seek_position + segment_data_position)),
                                    Err(e) => {
                                        log::warn!("Failed to read seek_id as VInt: {e}, skip...");
                                        None
                                    }
                                }
                            })
                        })
                        .collect();

                    seeks.sort_by_key(|a| a.1);

                    // find position larger than first_cluster_position
                    if let Some(pos) = seeks.iter().find(|(_, pos)| *pos > first_cluster_position) {
                        reader.seek(std::io::SeekFrom::Start(pos.1)).await?;
                        continue;
                    }

                    if segment_header.size.is_unknown {
                        break;
                    } else {
                        let eos = segment_data_position + *segment_header.size;
                        reader.seek(std::io::SeekFrom::Start(eos)).await?;
                        continue;
                    }
                }
                Segment::ID => {
                    out.push(SegmentView {
                        seek_head: take(&mut seek_head),
                        // Info is required in a valid Matroska file
                        info: info.take().ok_or(crate::Error::MissingElement(Info::ID))?,
                        tracks: tracks.take(),
                        cues: cues.take(),
                        attachments: attachments.take(),
                        chapters: chapters.take(),
                        tags: take(&mut tags),
                        first_cluster_position: take(&mut first_cluster_position),
                        segment_data_position: take(&mut segment_data_position),
                    });
                    segment_data_position = reader.stream_position().await?;
                }
                _ => {
                    use log::warn;
                    use tokio::io::AsyncReadExt;
                    // Skip unknown elements, here we read and discard the data for efficiency
                    tokio::io::copy(&mut reader.take(*header.size), &mut tokio::io::sink()).await?;
                    warn!("Skipped unknown element with ID: {}", header.id);
                }
            }
        }

        // Info is required in a valid Matroska file
        let info = info.ok_or(crate::Error::MissingElement(Info::ID))?;

        out.push(SegmentView {
            seek_head,
            info,
            tracks,
            cues,
            attachments,
            chapters,
            tags,
            first_cluster_position,
            segment_data_position,
        });
        Ok(out)
    }
}
