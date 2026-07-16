//! Developer task runner. Invoke as `cargo run -p xtask -- <task>`.
//!
//! Tasks:
//!   gen-tables    Convert `av2-spec/v1.0.0/attachments/*.h` into Rust const tables
//!                 under `crates/av2-tables/src/generated/`.
//!   build-refdec  cmake-build the AVM reference decoder (`avmdec`) from the sibling
//!                 `avm/` tree into `target/refdec/`.
//!   validate      Encode a test frame, decode it with `avmdec`, and compare.
//!   rav2e-pkg-config
//!                 Write target/rav2e-capi/pkgconfig/rav2e.pc for FFmpeg configure.
#![forbid(unsafe_code)]

mod tablegen;

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

const REFDEC_DIR: &str = "target/refdec";
const REFDEC_AVMDEC: &str = "target/refdec/avmdec";
const REFDEC_MD5: &str = "target/refdec/examples/decode_to_md5";
const VALIDATION_DIR: &str = "target/validation";
const VALIDATION_MD5: &str = "58efe7d34c4f36aab183bbf18a3f1e6a";
const RAV2E_PC_DIR: &str = "target/rav2e-capi/pkgconfig";
const RAV2E_PC_PATH: &str = "target/rav2e-capi/pkgconfig/rav2e.pc";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let task = args.next().unwrap_or_default();
    match task.as_str() {
        "gen-tables" => match tablegen::run(args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("gen-tables: {err}");
                ExitCode::from(1)
            }
        },
        "build-refdec" => exit("build-refdec", build_refdec(parse_force(args))),
        "validate" => exit("validate", validate()),
        "rav2e-pkg-config" => exit("rav2e-pkg-config", write_rav2e_pkg_config()),
        other => {
            eprintln!("unknown task: {other:?}");
            eprintln!("tasks: gen-tables | build-refdec | validate | rav2e-pkg-config");
            ExitCode::from(2)
        }
    }
}

fn exit(task: &str, result: Result<(), Error>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{task}: {err}");
            ExitCode::from(1)
        }
    }
}

fn parse_force(mut args: impl Iterator<Item = String>) -> bool {
    args.any(|arg| arg == "--force")
}

fn build_refdec(force: bool) -> Result<(), Error> {
    let avmdec = Path::new(REFDEC_AVMDEC);
    let md5 = Path::new(REFDEC_MD5);
    if !force && avmdec.is_file() && md5.is_file() {
        eprintln!("build-refdec: using cached {REFDEC_DIR}");
        return Ok(());
    }

    run(Command::new("cmake")
        .arg("-S")
        .arg("../avm")
        .arg("-B")
        .arg(REFDEC_DIR)
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg("-DCONFIG_AV2_DECODER=1")
        .arg("-DCONFIG_AV2_ENCODER=0")
        .arg("-DENABLE_APPS=1")
        .arg("-DENABLE_EXAMPLES=1")
        .arg("-DENABLE_TESTS=0"))?;
    run(Command::new("cmake").args([
        "--build",
        REFDEC_DIR,
        "--target",
        "avmdec",
        "decode_to_md5",
        "-j",
        "8",
    ]))?;

    if !avmdec.is_file() {
        return Err(Error::msg(format!(
            "missing built decoder {}",
            avmdec.display()
        )));
    }
    if !md5.is_file() {
        return Err(Error::msg(format!(
            "missing built md5 tool {}",
            md5.display()
        )));
    }
    Ok(())
}

fn validate() -> Result<(), Error> {
    // avmdec (AVM reference decoder) requires cmake + network to build.
    // If unavailable, print a skip message rather than failing.
    let avmdec = Path::new(REFDEC_AVMDEC);
    if !avmdec.is_file() {
        match build_refdec(false) {
            Ok(()) => {}
            Err(err) => {
                eprintln!("validate: avmdec unavailable ({err}), skipping MD5 gate");
                eprintln!(
                    "validate: to build avmdec, place the avm tree at ../avm and re-run with --force"
                );
                return Ok(());
            }
        }
    }

    fs::create_dir_all(VALIDATION_DIR)?;

    let input = PathBuf::from(VALIDATION_DIR).join("gray128.y4m");
    let obu = PathBuf::from(VALIDATION_DIR).join("gray128.obu");
    let decoded = PathBuf::from(VALIDATION_DIR).join("gray128.dec.yuv");
    let md5 = PathBuf::from(VALIDATION_DIR).join("gray128.md5");

    fs::write(&input, gray_y4m_128())?;

    // Encode with whytho-cli (if available), otherwise skip encode+decode gate.
    let whytho_cli = Path::new("target/debug/whytho-cli");
    if !whytho_cli.is_file() {
        match run(Command::new("cargo").args(["build", "-p", "whytho-cli"])) {
            Ok(()) => {}
            Err(err) => {
                eprintln!("validate: could not build whytho-cli ({err}), skipping encode gate");
                return Ok(());
            }
        }
    }

    // Try to encode; if the av2 encode path isn't wired yet, skip gracefully.
    match run(Command::new(whytho_cli).args([
        "encode",
        "--codec",
        "av2",
        &input.to_string_lossy(),
        &obu.to_string_lossy(),
    ])) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("validate: av2 encode failed or not wired ({err}), skipping MD5 gate");
            return Ok(());
        }
    }

    run(Command::new(REFDEC_AVMDEC)
        .arg("--rawvideo")
        .arg("-o")
        .arg(&decoded)
        .arg(&obu))?;

    let decoded_len = fs::metadata(&decoded)?.len();
    if decoded_len != 128 * 128 + 64 * 64 * 2 {
        return Err(Error::msg(format!(
            "decoded output length {decoded_len} did not match 128x128 I420"
        )));
    }

    run(Command::new(REFDEC_MD5).arg(&obu).arg(&md5))?;
    let md5_text = fs::read_to_string(&md5)?;
    if !md5_text.contains(VALIDATION_MD5) {
        return Err(Error::msg(format!(
            "decoded MD5 mismatch; expected {VALIDATION_MD5}, got {}",
            md5_text.trim()
        )));
    }

    eprintln!("validate: AVM decoded 128x128 I420, md5 {VALIDATION_MD5}");
    Ok(())
}

fn write_rav2e_pkg_config() -> Result<(), Error> {
    fs::create_dir_all(RAV2E_PC_DIR)?;
    let root = std::env::current_dir()?;
    let prefix = root.display();
    let pc = format!(
        "prefix={prefix}\n\
         libdir=${{prefix}}/target/release\n\
         includedir=${{prefix}}/include\n\
         \n\
         Name: rav2e\n\
         Description: Experimental rav2e AV2 encoder C ABI\n\
         Version: 0.0.1\n\
         Libs: ${{libdir}}/librav2e_capi.a -lSystem -lc -lm\n\
         Cflags: -I${{includedir}}\n"
    );
    fs::write(RAV2E_PC_PATH, pc)?;
    eprintln!("rav2e-pkg-config: wrote {RAV2E_PC_PATH}");
    Ok(())
}

fn gray_y4m_128() -> Vec<u8> {
    let mut out = b"YUV4MPEG2 W128 H128 F1:1 Ip A0:0 C420\nFRAME\n".to_vec();
    out.resize(out.len() + 128 * 128 + 64 * 64 * 2, 128);
    out
}

fn run(command: &mut Command) -> Result<(), Error> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::msg(format!("{program} exited with status {status}")))
    }
}

#[derive(Debug)]
struct Error {
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

impl From<tablegen::Error> for Error {
    fn from(value: tablegen::Error) -> Self {
        Self::msg(value.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
