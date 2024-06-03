#![allow(unused)]

use std::{env, fs, path::Path, process::Command};

use anyhow::{anyhow, Result};

fn is_exsit(dir: &str) -> bool {
    fs::metadata(dir).is_ok()
}

fn join(root: &str, next: &str) -> String {
    Path::new(root).join(next).to_str().unwrap().to_string()
}

fn exec(command: &str, work_dir: &str) -> anyhow::Result<String> {
    let output = Command::new(if cfg!(windows) { "powershell" } else { "bash" })
        .arg(if cfg!(windows) { "-command" } else { "-c" })
        .arg(if cfg!(windows) {
            format!("$ProgressPreference = 'SilentlyContinue';{}", command)
        } else {
            command.to_string()
        })
        .current_dir(work_dir)
        .output()?;
    if !output.status.success() {
        Err(anyhow!("{}", unsafe {
            String::from_utf8_unchecked(output.stderr)
        }))
    } else {
        Ok(unsafe { String::from_utf8_unchecked(output.stdout) })
    }
}

fn main() -> anyhow::Result<()> {
    let target = env::var("TARGET")?;
    let out_dir = env::var("OUT_DIR")?;
    let is_debug = env::var("DEBUG")
        .map(|label| label == "true")
        .unwrap_or(true);

    if target.find("android").is_some() {
        if !is_exsit(&join(&out_dir, "libsrt.a")) {
            exec(
                "wget \
                -O libsrt.a \
                https://github.com/mycrl/distributions/releases/download/distributions/libsrt-arm64-v8a.a",
                &out_dir,
            )?;
        }

        if !is_exsit(&join(&out_dir, "libssl.a")) {
            exec(
                "wget \
                -O libssl.a \
                https://github.com/mycrl/distributions/releases/download/distributions/libssl-arm64-v8a.a",
                &out_dir,
            )?;
        }

        if !is_exsit(&join(&out_dir, "libcrypto.a")) {
            exec(
                "wget \
                -O libcrypto.a \
                https://github.com/mycrl/distributions/releases/download/distributions/libcrypto-arm64-v8a.a",
                &out_dir,
            )?;
        }

        println!("cargo:rustc-link-search=all={}", out_dir);
        println!("cargo:rustc-link-lib=static=srt");
        println!("cargo:rustc-link-lib=static=ssl");
        println!("cargo:rustc-link-lib=static=crypto");
        println!("cargo:rustc-link-lib=c++");
    } else {
        let srt_dir = join(&out_dir, "srt");
        if !is_exsit(&srt_dir) {
            exec(
                "git clone --branch v1.5.3 https://github.com/Haivision/srt",
                &out_dir,
            )?;
        }

        if !is_exsit(&join(
            &srt_dir,
            if cfg!(windows) {
                "./Release/srt_static.lib"
            } else {
                "libsrt.a"
            },
        )) {
            #[cfg(target_os = "linux")]
            {
                exec("./configure", &srt_dir)?;
                exec("make", &srt_dir)?;
            }

            #[cfg(not(target_os = "linux"))]
            {
                exec(
                    &format!(
                        "cmake {} .",
                        [
                            &format!("-DENABLE_DEBUG={}", if is_debug { "ON" } else { "OFF" }),
                            "-DCMAKE_BUILD_TYPE=Release",
                            "-DENABLE_APPS=OFF",
                            "-DENABLE_BONDING=ON",
                            "-DENABLE_CODE_COVERAGE=OFF",
                            "-DENABLE_SHARED=OFF",
                            "-DENABLE_STATIC=ON",
                            "-DENABLE_ENCRYPTION=OFF",
                            "-DENABLE_UNITTESTS=OFF",
                            "-DENABLE_STDCXX_SYNC=ON"
                        ]
                        .join(" ")
                    ),
                    &srt_dir,
                )?;

                exec("cmake --build . --config Release", &srt_dir)?;
            }
        }

        #[cfg(target_os = "windows")]
        {
            println!(
                "cargo:rustc-link-search=all={}",
                join(&srt_dir, "./Release")
            );

            println!("cargo:rustc-link-lib=srt_static");
        }

        #[cfg(not(target_os = "windows"))]
        {
            println!("cargo:rustc-link-search=all={}", srt_dir);
            println!("cargo:rustc-link-lib=srt");

            #[cfg(target_os = "macos")]
            println!("cargo:rustc-link-lib=c++");

            #[cfg(target_os = "linux")]
            println!("cargo:rustc-link-lib=stdc++");
        }
    }

    Ok(())
}
