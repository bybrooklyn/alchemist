use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fmt, fs, io,
    path::{Path, PathBuf},
};

const ATTACHMENTS_DIR: &str = "whytho/av2-spec/v1.0.0/attachments";
const GENERATED_DIR: &str = "whytho/crates/whytho-tables/src/generated";
const EXPECTED_HEADER_COUNT: usize = 245;

pub fn run(args: impl Iterator<Item = String>) -> Result<(), Error> {
    let mut check = false;
    for arg in args {
        match arg.as_str() {
            "--check" => check = true,
            _ => return Err(Error::msg(format!("unknown gen-tables argument: {arg}"))),
        }
    }

    let generated = generate_all(Path::new(ATTACHMENTS_DIR))?;
    let out_dir = Path::new(GENERATED_DIR);
    if check {
        check_generated(out_dir, &generated)
    } else {
        write_generated(out_dir, &generated)
    }
}

fn generate_all(attachments_dir: &Path) -> Result<BTreeMap<PathBuf, String>, Error> {
    let mut headers = Vec::new();
    for entry in fs::read_dir(attachments_dir)? {
        let path = entry?.path();
        if path.extension() == Some(OsStr::new("h"))
            && path.file_name() != Some(OsStr::new("all_tables.h"))
        {
            headers.push(path);
        }
    }
    headers.sort();
    if headers.len() != EXPECTED_HEADER_COUNT {
        return Err(Error::msg(format!(
            "expected {EXPECTED_HEADER_COUNT} individual headers, found {}",
            headers.len()
        )));
    }

    let mut generated = BTreeMap::new();
    let mut module_names = Vec::new();
    for path in headers {
        let source = fs::read_to_string(&path)?;
        let table = parse_table(&source, &path)?;
        let module_name = module_name(&table.name);
        let rust = emit_table(&table, &path)?;
        generated.insert(PathBuf::from(format!("{module_name}.rs")), rust);
        module_names.push(module_name);
    }

    generated.insert(PathBuf::from("mod.rs"), emit_mod(&module_names));
    Ok(generated)
}

fn check_generated(out_dir: &Path, generated: &BTreeMap<PathBuf, String>) -> Result<(), Error> {
    let mut mismatches = Vec::new();
    for (relative, expected) in generated {
        let path = out_dir.join(relative);
        let actual = fs::read_to_string(&path).unwrap_or_default();
        if actual != *expected {
            mismatches.push(path);
        }
    }

    for entry in fs::read_dir(out_dir)? {
        let path = entry?.path();
        if path.extension() == Some(OsStr::new("rs")) {
            let relative = PathBuf::from(path.file_name().expect("generated file has a name"));
            if !generated.contains_key(&relative) {
                mismatches.push(path);
            }
        }
    }

    if mismatches.is_empty() {
        eprintln!("gen-tables --check: generated tables are up to date");
        Ok(())
    } else {
        mismatches.sort();
        let paths = mismatches
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ");
        Err(Error::msg(format!(
            "generated tables are stale; run `cargo run -p xtask -- gen-tables`\n  {paths}"
        )))
    }
}

fn write_generated(out_dir: &Path, generated: &BTreeMap<PathBuf, String>) -> Result<(), Error> {
    fs::create_dir_all(out_dir)?;

    for entry in fs::read_dir(out_dir)? {
        let path = entry?.path();
        if path.extension() == Some(OsStr::new("rs")) {
            let relative = PathBuf::from(path.file_name().expect("generated file has a name"));
            if !generated.contains_key(&relative) {
                fs::remove_file(path)?;
            }
        }
    }

    for (relative, contents) in generated {
        let path = out_dir.join(relative);
        if fs::read_to_string(&path).ok().as_deref() != Some(contents.as_str()) {
            fs::write(path, contents)?;
        }
    }

    eprintln!("gen-tables: generated {} Rust files", generated.len());
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Table {
    name: String,
    values: Node,
    shape: Vec<usize>,
    min: i64,
    max: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Array(Vec<Node>),
    Value(i64),
}

fn parse_table(source: &str, path: &Path) -> Result<Table, Error> {
    let stripped = strip_comments(source);
    let (lhs, rhs) = stripped
        .split_once('=')
        .ok_or_else(|| Error::msg(format!("{}: missing `=`", path.display())))?;
    let name = parse_name(lhs).ok_or_else(|| {
        Error::msg(format!(
            "{}: missing table name before initializer",
            path.display()
        ))
    })?;
    let mut parser = Parser::new(rhs, path);
    let values = parser.parse_initializer()?;
    let shape = values.shape().ok_or_else(|| {
        Error::msg(format!(
            "{}: initializer for {name} is not rectangular",
            path.display()
        ))
    })?;
    let (min, max) = values.range().ok_or_else(|| {
        Error::msg(format!(
            "{}: initializer for {name} has no values",
            path.display()
        ))
    })?;

    Ok(Table {
        name,
        values,
        shape,
        min,
        max,
    })
}

fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn parse_name(lhs: &str) -> Option<String> {
    let trimmed = lhs.trim_start();
    let end = trimmed
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(trimmed.len());
    (end > 0).then(|| trimmed[..end].to_string())
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    path: &'a Path,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, path: &'a Path) -> Self {
        Self {
            bytes: source.as_bytes(),
            pos: 0,
            path,
        }
    }

    fn parse_initializer(&mut self) -> Result<Node, Error> {
        self.skip_ws();
        let node = self.parse_array()?;
        self.skip_ws();
        if self.peek() == Some(b';') {
            self.pos += 1;
            self.skip_ws();
        }
        if self.pos != self.bytes.len() {
            return Err(self.err("trailing tokens after initializer"));
        }
        Ok(node)
    }

    fn parse_array(&mut self) -> Result<Node, Error> {
        self.expect(b'{')?;
        let mut values = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Node::Array(values));
                }
                Some(b'{') => values.push(self.parse_array()?),
                Some(_) => values.push(Node::Value(self.parse_value()?)),
                None => return Err(self.err("unterminated initializer")),
            }

            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {}
                Some(_) => return Err(self.err("expected `,` or `}`")),
                None => return Err(self.err("unterminated initializer")),
            }
        }
    }

    fn parse_value(&mut self) -> Result<i64, Error> {
        self.skip_ws();
        if matches!(self.peek(), Some(b'-' | b'+') | Some(b'0'..=b'9')) {
            self.parse_number()
        } else {
            let ident = self.parse_ident()?;
            resolve_symbol(&ident).ok_or_else(|| {
                self.err(format!(
                    "unknown symbolic table value `{ident}`; add it to tablegen.rs"
                ))
            })
        }
    }

    fn parse_number(&mut self) -> Result<i64, Error> {
        let mut sign = 1_i64;
        if let Some(byte @ (b'-' | b'+')) = self.peek() {
            if byte == b'-' {
                sign = -1;
            }
            self.pos += 1;
            self.skip_ws();
        }
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(self.err("expected digits after numeric sign"));
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).expect("ascii number");
        text.parse::<i64>()
            .map(|value| sign * value)
            .map_err(|_| self.err(format!("invalid number `{text}`")))
    }

    fn parse_ident(&mut self) -> Result<String, Error> {
        let start = self.pos;
        if !matches!(self.peek(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_')) {
            return Err(self.err("expected value"));
        }
        self.pos += 1;
        while matches!(
            self.peek(),
            Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
        ) {
            self.pos += 1;
        }
        Ok(std::str::from_utf8(&self.bytes[start..self.pos])
            .expect("ascii identifier")
            .to_string())
    }

    fn expect(&mut self, byte: u8) -> Result<(), Error> {
        self.skip_ws();
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(format!("expected `{}`", byte as char)))
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        Error::msg(format!(
            "{}:{}: {}",
            self.path.display(),
            self.pos,
            msg.into()
        ))
    }
}

impl Node {
    fn shape(&self) -> Option<Vec<usize>> {
        match self {
            Node::Value(_) => Some(Vec::new()),
            Node::Array(items) => {
                let mut shape = vec![items.len()];
                let mut child_shape: Option<Vec<usize>> = None;
                for item in items {
                    let current = item.shape()?;
                    if let Some(expected) = &child_shape {
                        if expected != &current {
                            return None;
                        }
                    } else {
                        child_shape = Some(current);
                    }
                }
                shape.extend(child_shape.unwrap_or_default());
                Some(shape)
            }
        }
    }

    fn range(&self) -> Option<(i64, i64)> {
        match self {
            Node::Value(value) => Some((*value, *value)),
            Node::Array(items) => {
                let mut min = i64::MAX;
                let mut max = i64::MIN;
                let mut seen = false;
                for item in items {
                    if let Some((item_min, item_max)) = item.range() {
                        min = min.min(item_min);
                        max = max.max(item_max);
                        seen = true;
                    }
                }
                seen.then_some((min, max))
            }
        }
    }
}

fn resolve_symbol(symbol: &str) -> Option<i64> {
    let value = match symbol {
        "BLOCK_4X4" => 0,
        "BLOCK_4X8" => 1,
        "BLOCK_8X4" => 2,
        "BLOCK_8X8" => 3,
        "BLOCK_8X16" => 4,
        "BLOCK_16X8" => 5,
        "BLOCK_16X16" => 6,
        "BLOCK_16X32" => 7,
        "BLOCK_32X16" => 8,
        "BLOCK_32X32" => 9,
        "BLOCK_32X64" => 10,
        "BLOCK_64X32" => 11,
        "BLOCK_64X64" => 12,
        "BLOCK_64X128" => 13,
        "BLOCK_128X64" => 14,
        "BLOCK_128X128" => 15,
        "BLOCK_128X256" => 16,
        "BLOCK_256X128" => 17,
        "BLOCK_256X256" => 18,
        "BLOCK_4X16" => 19,
        "BLOCK_16X4" => 20,
        "BLOCK_8X32" => 21,
        "BLOCK_32X8" => 22,
        "BLOCK_16X64" => 23,
        "BLOCK_64X16" => 24,
        "BLOCK_4X32" => 25,
        "BLOCK_32X4" => 26,
        "BLOCK_8X64" => 27,
        "BLOCK_64X8" => 28,
        "BLOCK_INVALID" => 255,
        "TX_4X4" => 0,
        "TX_8X8" => 1,
        "TX_16X16" => 2,
        "TX_32X32" => 3,
        "TX_64X64" => 4,
        "TX_4X8" => 5,
        "TX_8X4" => 6,
        "TX_8X16" => 7,
        "TX_16X8" => 8,
        "TX_16X32" => 9,
        "TX_32X16" => 10,
        "TX_32X64" => 11,
        "TX_64X32" => 12,
        "TX_4X16" => 13,
        "TX_16X4" => 14,
        "TX_8X32" => 15,
        "TX_32X8" => 16,
        "TX_16X64" => 17,
        "TX_64X16" => 18,
        "TX_4X32" => 19,
        "TX_32X4" => 20,
        "TX_8X64" => 21,
        "TX_64X8" => 22,
        "DCT_DCT" => 0,
        "ADST_DCT" => 1,
        "DCT_ADST" => 2,
        "ADST_ADST" => 3,
        "reserved" => 0,
        _ => return None,
    };
    Some(value)
}

fn emit_table(table: &Table, source_path: &Path) -> Result<String, Error> {
    let rust_name = const_name(&table.name);
    let rust_type = integer_type(&table.name, table.min, table.max)?;
    let array_type = array_type(&rust_type, &table.shape);
    let source_name = source_path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("unknown");
    let mut out = String::new();
    out.push_str("// @generated by `cargo run -p xtask -- gen-tables`; do not edit.\n");
    out.push_str(&format!("// Source: {source_name}\n\n"));
    out.push_str("#[rustfmt::skip]\n");
    out.push_str(&format!("pub static {rust_name}: {array_type} = "));
    emit_node(&table.values, &mut out, 0);
    out.push_str(";\n");
    Ok(out)
}

fn emit_mod(module_names: &[String]) -> String {
    let mut out = String::new();
    out.push_str("// @generated by `cargo run -p xtask -- gen-tables`; do not edit.\n");
    out.push_str("// Source: av2-spec/v1.0.0/attachments/*.h excluding all_tables.h\n\n");
    for module_name in module_names {
        out.push_str(&format!("pub mod {module_name};\n"));
    }
    out
}

fn emit_node(node: &Node, out: &mut String, indent: usize) {
    match node {
        Node::Value(value) => out.push_str(&value.to_string()),
        Node::Array(items) => {
            if items.iter().all(|item| matches!(item, Node::Value(_))) {
                emit_value_array(items, out, indent);
            } else {
                out.push_str("[\n");
                for item in items {
                    out.push_str(&" ".repeat(indent + 4));
                    emit_node(item, out, indent + 4);
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(indent));
                out.push(']');
            }
        }
    }
}

fn emit_value_array(items: &[Node], out: &mut String, indent: usize) {
    if items.len() <= 16 {
        out.push('[');
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            if let Node::Value(value) = item {
                out.push_str(&value.to_string());
            }
        }
        out.push(']');
    } else {
        out.push_str("[\n");
        for chunk in items.chunks(16) {
            out.push_str(&" ".repeat(indent + 4));
            for item in chunk {
                if let Node::Value(value) = item {
                    out.push_str(&value.to_string());
                    out.push_str(", ");
                }
            }
            out.push('\n');
        }
        out.push_str(&" ".repeat(indent));
        out.push(']');
    }
}

fn integer_type(name: &str, min: i64, max: i64) -> Result<String, Error> {
    if name.ends_with("_Cdf") || name.ends_with("_CDF") || name.contains("_Cdf") {
        if min < 0 || max > u16::MAX as i64 {
            return Err(Error::msg(format!(
                "{name}: CDF values do not fit u16: {min}..={max}"
            )));
        }
        return Ok("u16".to_string());
    }

    let ty = if min < 0 {
        if min >= i8::MIN as i64 && max <= i8::MAX as i64 {
            "i8"
        } else if min >= i16::MIN as i64 && max <= i16::MAX as i64 {
            "i16"
        } else if min >= i32::MIN as i64 && max <= i32::MAX as i64 {
            "i32"
        } else {
            "i64"
        }
    } else if max <= u8::MAX as i64 {
        "u8"
    } else if max <= u16::MAX as i64 {
        "u16"
    } else if max <= u32::MAX as i64 {
        "u32"
    } else {
        "u64"
    };
    Ok(ty.to_string())
}

fn array_type(element: &str, shape: &[usize]) -> String {
    let mut ty = element.to_string();
    for dim in shape.iter().rev() {
        ty = format!("[{ty}; {dim}]");
    }
    ty
}

fn const_name(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower_or_digit = false;
    for ch in name.chars() {
        if ch == '_' {
            out.push('_');
            prev_lower_or_digit = false;
        } else if ch.is_ascii_uppercase() {
            if prev_lower_or_digit {
                out.push('_');
            }
            out.push(ch);
            prev_lower_or_digit = false;
        } else {
            out.push(ch.to_ascii_uppercase());
            prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    out
}

fn module_name(name: &str) -> String {
    const_name(name).to_ascii_lowercase()
}

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    fn msg(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::msg(value.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_header_and_derives_shape() {
        let table = parse_table(
            "Default_Coeff_Base_Cdf\n[ COEFF_CDF_Q_CTXS ][ 1 ] = {\n  {{1, 2, 3}},\n  {{4, 5, 6}}\n}",
            Path::new("coeff.h"),
        )
        .unwrap();

        assert_eq!(table.name, "Default_Coeff_Base_Cdf");
        assert_eq!(table.shape, vec![2, 1, 3]);
        assert_eq!((table.min, table.max), (1, 6));
        assert_eq!(
            integer_type(&table.name, table.min, table.max).unwrap(),
            "u16"
        );
    }

    #[test]
    fn resolves_symbolic_values() {
        let table = parse_table(
            "Mixed[3][4] = {{BLOCK_4X4, BLOCK_128X256, BLOCK_INVALID, reserved}, {TX_4X4, TX_64X8, DCT_DCT, ADST_ADST}, {DCT_ADST, ADST_DCT, TX_32X4, BLOCK_8X64}}",
            Path::new("symbols.h"),
        )
        .unwrap();

        assert_eq!(table.shape, vec![3, 4]);
        assert_eq!(table.min, 0);
        assert_eq!(table.max, 255);
        assert_eq!(
            table.values,
            Node::Array(vec![
                Node::Array(vec![
                    Node::Value(0),
                    Node::Value(16),
                    Node::Value(255),
                    Node::Value(0),
                ]),
                Node::Array(vec![
                    Node::Value(0),
                    Node::Value(22),
                    Node::Value(0),
                    Node::Value(3),
                ]),
                Node::Array(vec![
                    Node::Value(2),
                    Node::Value(1),
                    Node::Value(20),
                    Node::Value(27),
                ]),
            ])
        );
    }

    #[test]
    fn spot_checks_dct_kernel_emit() {
        let table = parse_table(
            include_str!("../../av2-spec/v1.0.0/attachments/dct_kernel4.h"),
            Path::new("dct_kernel4.h"),
        )
        .unwrap();

        assert_eq!(table.name, "Dct_Kernel4");
        assert_eq!(table.shape, vec![4, 4]);
        assert_eq!(
            integer_type(&table.name, table.min, table.max).unwrap(),
            "i8"
        );
        match table.values {
            Node::Array(rows) => assert_eq!(
                rows[1],
                Node::Array(vec![
                    Node::Value(83),
                    Node::Value(35),
                    Node::Value(-35),
                    Node::Value(-83),
                ])
            ),
            Node::Value(_) => panic!("expected array"),
        }
    }
}
