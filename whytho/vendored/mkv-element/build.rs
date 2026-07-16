use std::env;
use std::fs;
use std::path::Path;

use askama::Template;

#[derive(Template)]
#[template(path = "unsigned.txt")]
struct UnsignedTmpl<'a> {
    doc: &'a str,
    name: &'a str,
    id: &'a str,
    default_value: &'a str,
    has_default: bool,
}

#[derive(Template)]
#[template(path = "signed.txt")]
struct SignedTmpl<'a> {
    doc: &'a str,
    name: &'a str,
    id: &'a str,
    default_value: &'a str,
    has_default: bool,
}

#[derive(Template)]
#[template(path = "float.txt")]
struct FloatTmpl<'a> {
    doc: &'a str,
    name: &'a str,
    id: &'a str,
    default_value: &'a str,
    has_default: bool,
}

#[derive(Template)]
#[template(path = "text.txt")]
struct TextTmpl<'a> {
    doc: &'a str,
    name: &'a str,
    id: &'a str,
    default_value: &'a str,
    has_default: bool,
}

#[derive(Template)]
#[template(path = "bin.txt")]
struct BinTmpl<'a> {
    doc: &'a str,
    name: &'a str,
    id: &'a str,
}

#[derive(Template)]
#[template(path = "date.txt")]
struct DateTmpl<'a> {
    doc: &'a str,
    name: &'a str,
    id: &'a str,
    default_value: &'a str,
    has_default: bool,
}

/// Format documentation text as `/// ` prefixed lines.
fn format_doc(text: &str) -> String {
    text.lines()
        .map(|line| {
            format!(
                "/// {}",
                line.trim().replace("[", "\\[").replace("]", "\\]")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Adjust element names from the XML specification to Rust naming conventions.
fn adjust_name(name: &str) -> &str {
    match name {
        "EBMLMaxIDLength" => "EbmlMaxIdLength",
        "EBMLMaxSizeLength" => "EbmlMaxSizeLength",
        "SeekID" => "SeekId",
        "SegmentUUID" => "SegmentUuid",
        "PrevUUID" => "PrevUuid",
        "NextUUID" => "NextUuid",
        "DateUTC" => "DateUtc",
        "ChapterTranslateID" => "ChapterTranslateId",
        "ChapterTranslateEditionUID" => "ChapterTranslateEditionUid",
        "BlockAddID" => "BlockAddId",
        "TrackUID" => "TrackUid",
        "LanguageBCP47" => "LanguageBcp47",
        "CodecID" => "CodecId",
        "MaxBlockAdditionID" => "MaxBlockAdditionId",
        "BlockAddIDType" => "BlockAddIdType",
        "BlockAddIDValue" => "BlockAddIdValue",
        "BlockAddIDExtraData" => "BlockAddIdExtraData",
        "BlockAddIDName" => "BlockAddIdName",
        "TrackTranslateTrackID" => "TrackTranslateTrackId",
        "TrackTranslateEditionUID" => "TrackTranslateEditionUid",
        "UncompressedFourCC" => "UncompressedFourcc",
        "MaxCLL" => "MaxCll",
        "MaxFALL" => "MaxFall",
        "TrackPlaneUID" => "TrackPlaneUid",
        "TrackJoinUID" => "TrackJoinUid",
        "ContentEncKeyID" => "ContentEncKeyId",
        "AESSettingsCipherMode" => "AesSettingsCipherMode",
        "FileUID" => "FileUid",
        "EditionUID" => "EditionUid",
        "EditionLanguageIETF" => "EditionLanguageIetf",
        "ChapterUID" => "ChapterUid",
        "ChapterStringUID" => "ChapterStringUid",
        "ChapterSegmentUID" => "ChapterSegmentUid",
        "ChapterSegmentUUID" => "ChapterSegmentUuid",
        "ChapterSegmentEditionUID" => "ChapterSegmentEditionUid",
        "ChapterTrackUID" => "ChapterTrackUid",
        "ChapLanguageBCP47" => "ChapLanguageBcp47",
        "ChapProcessCodecID" => "ChapProcessCodecId",
        "TagTrackUID" => "TagTrackUid",
        "TagEditionUID" => "TagEditionUid",
        "TagChapterUID" => "TagChapterUid",
        "TagAttachmentUID" => "TagAttachmentUid",
        "TagLanguageBCP47" => "TagLanguageBcp47",
        _ => name,
    }
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_types.rs");

    let content = fs::read_to_string("matroska-specification/ebml_matroska.xml").unwrap();
    let xml = roxmltree::Document::parse(&content).unwrap();

    let mut output = String::new();

    for element in xml
        .descendants()
        .filter(|n| n.has_tag_name("element"))
        .filter(|n| n.attribute("type") != Some("master"))
    {
        let raw_name = element.attribute("name").unwrap();
        let id = element.attribute("id").unwrap();
        let default_value = element.attribute("default");
        let has_default = default_value.is_some();
        let default_value = default_value.unwrap_or("0");

        let doc = element
            .children()
            .find(|n| n.has_tag_name("documentation"))
            .and_then(|n| n.text())
            .map(format_doc)
            .unwrap_or_else(|| format!("/// {raw_name} in ebml"));

        let name = adjust_name(raw_name);

        let rendered = match element.attribute("type").unwrap() {
            "uinteger" => UnsignedTmpl {
                doc: &doc,
                name,
                id,
                default_value,
                has_default,
            }
            .render()
            .unwrap(),
            "integer" => SignedTmpl {
                doc: &doc,
                name,
                id,
                default_value,
                has_default,
            }
            .render()
            .unwrap(),
            "float" => FloatTmpl {
                doc: &doc,
                name,
                id,
                default_value,
                has_default,
            }
            .render()
            .unwrap(),
            "string" | "utf-8" => TextTmpl {
                doc: &doc,
                name,
                id,
                default_value,
                has_default,
            }
            .render()
            .unwrap(),
            "binary" => BinTmpl {
                doc: &doc,
                name,
                id,
            }
            .render()
            .unwrap(),
            "date" => DateTmpl {
                doc: &doc,
                name,
                id,
                default_value,
                has_default,
            }
            .render()
            .unwrap(),
            other => panic!("Unknown type: {other}"),
        };
        output.push_str(&rendered);
        output.push('\n');
    }

    // Additional EBML header elements
    let extra_elements: &[(&str, &str, &str, &str)] = &[
        (
            "EbmlVersion",
            "0x4286",
            "1",
            "/// EBMLVersion element, indicates the version of EBML used.",
        ),
        (
            "EbmlReadVersion",
            "0x42f7",
            "1",
            "/// EBMLReadVersion element, indicates the read version of EBML used.",
        ),
        (
            "DocType",
            "0x4282",
            "matroska",
            "/// DocType element, indicates the type of the document.",
        ),
        (
            "DocTypeVersion",
            "0x4287",
            "1",
            "/// DocTypeVersion element, indicates the version of the document type.",
        ),
        (
            "DocTypeReadVersion",
            "0x4285",
            "1",
            "/// DocTypeReadVersion element, indicates the read version of the document type.",
        ),
    ];

    for &(name, id, default_value, doc) in extra_elements {
        let rendered = match name {
            "DocType" => TextTmpl {
                doc,
                name,
                id,
                default_value,
                has_default: true,
            }
            .render()
            .unwrap(),
            _ => UnsignedTmpl {
                doc,
                name,
                id,
                default_value,
                has_default: true,
            }
            .render()
            .unwrap(),
        };
        output.push_str(&rendered);
        output.push('\n');
    }

    fs::write(&dest_path, output).unwrap();
}
