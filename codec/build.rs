use std::{env, path::Path};

use anyhow::anyhow;
use dotenv::dotenv;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=./core/src");
    println!("cargo:rerun-if-changed=./build.rs");

    #[cfg(not(target_os = "linux"))]
    {
        let settings = Settings::build()?;
        compile_lib(&settings)?;
        link_lib(&settings);
    }
    
    Ok(())
}

fn link_lib(settings: &Settings) {
    println!("cargo:rustc-link-search=all={}", &settings.out_dir);
    println!("cargo:rustc-link-lib=codec");
}

fn compile_lib(settings: &Settings) -> anyhow::Result<()> {
    cc::Build::new()
        .cpp(false)
        .debug(settings.is_debug)
        .static_crt(true)
        .target(&settings.target)
        .warnings(false)
        .out_dir(&settings.out_dir)
        .file("./core/src/video_encoder.c")
        .include(&join(&settings.ffmpeg_prefix, "./include")?)
        .compile("codec");
    Ok(())
}

struct Settings {
    is_debug: bool,
    target: String,
    out_dir: String,
    ffmpeg_prefix: String,
}

impl Settings {
    fn build() -> anyhow::Result<Self> {
        let _ = dotenv();

        Ok(Self {
            ffmpeg_prefix: env::var("FFMPEG_PREFIX")?,
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
        .ok_or_else(|| anyhow!("failed to path into str!"))?
        .to_string())
}
