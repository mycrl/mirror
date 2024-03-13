use std::{env, fs, path::Path, process::Command};

use anyhow::anyhow;
use dotenv::dotenv;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=./lib");
    println!("cargo:rerun-if-changed=./build.rs");

    let settings = Settings::build()?;
    compile_lib(&settings)?;
    link_lib(&settings);
    Ok(())
}

fn link_lib(settings: &Settings) {
    println!("cargo:rustc-link-search=all={}", &settings.out_dir);
    println!("cargo:rustc-link-lib=codec");
}

fn compile_lib(settings: &Settings) -> anyhow::Result<()> {
    cc::Build::new()
        .cpp(false)
        .std("c17")
        .debug(settings.is_debug)
        .static_crt(true)
        .target(&settings.target)
        .warnings(false)
        .out_dir(&settings.out_dir)
        .file("./lib/devices.c")
        .includes(
            &settings
                .ffmpeg_prefix
                .clone()
                .map(|path| vec![join(&path, "./include").unwrap()])
                .unwrap_or_else(|| setup_dependencies(&settings)),
        )
        .compile("devices");
    Ok(())
}

fn setup_dependencies(settings: &Settings) -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec![
            join(exec("brew --prefix ffmpeg", &settings.out_dir).expect(
                "You don't have ffmpeg installed, please install ffmpeg: `brew install ffmpeg`.",
            ).trim(), "./include")
            .unwrap()
        ]
    } else if cfg!(target_os = "windows") {
        if is_exsit(&join(&settings.out_dir, "7z.exe").unwrap()) {
            exec(
                "Invoke-WebRequest -Uri https://www.7-zip.org/a/7zr.exe -OutFile 7z.exe",
                &settings.out_dir,
            )
            .expect("Unable to download 7z cli exe.");
        }

        let ffmpeg_prefix = join(&settings.out_dir, "ffmpeg-6.0-full_build-shared").unwrap();
        if is_exsit(&ffmpeg_prefix) {
            exec(
                "Invoke-WebRequest -Uri https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-6.0-full_build-shared.7z -OutFile ffmpeg.7z; \
                         ./7z.exe x ffmpeg.7z -aoa; \
                         del ffmpeg.7z",
                &settings.out_dir,
            )
            .expect("Unable to download ffmpeg shard release.");
        }

        vec![join(&ffmpeg_prefix, "./include").unwrap()]
    } else {
        let mut paths = Vec::with_capacity(10);
        paths.append(
            &mut pkg_config::probe_library("libavcodec")
                .expect("You don't have pck-config or libavcodec-dev installed.")
                .include_paths
                .iter()
                .map(|path| path.to_str().unwrap().to_string())
                .collect::<Vec<String>>(),
        );

        paths.append(
            &mut pkg_config::probe_library("libavutil")
                .expect("You don't have pck-config or libavutil-dev installed.")
                .include_paths
                .iter()
                .map(|path| path.to_str().unwrap().to_string())
                .collect::<Vec<String>>(),
        );

        paths
    }
}

struct Settings {
    is_debug: bool,
    target: String,
    out_dir: String,
    ffmpeg_prefix: Option<String>,
}

impl Settings {
    fn build() -> anyhow::Result<Self> {
        let _ = dotenv();

        Ok(Self {
            ffmpeg_prefix: env::var("FFMPEG_PREFIX").ok(),
            out_dir: env::var("OUT_DIR")?,
            target: env::var("TARGET")?,
            is_debug: env::var("DEBUG")
                .map(|label| label == "true")
                .unwrap_or(true),
        })
    }
}

fn join(root: &str, next: &str) -> anyhow::Result<String> {
    Ok(Path::new(root)
        .join(next)
        .to_str()
        .ok_or_else(|| anyhow!("Failed to path into string."))?
        .to_string())
}

fn is_exsit(dir: &str) -> bool {
    fs::metadata(dir).is_ok()
}

fn exec(command: &str, work_dir: &str) -> anyhow::Result<String> {
    let output = Command::new(if cfg!(windows) { "powershell" } else { "bash" })
        .arg(if cfg!(windows) { "-command" } else { "-c" })
        .arg(command)
        .current_dir(work_dir)
        .output()?;
    if !output.status.success() {
        Err(anyhow!("{}", String::from_utf8(output.stderr)?))
    } else {
        Ok(String::from_utf8(output.stdout)?)
    }
}
