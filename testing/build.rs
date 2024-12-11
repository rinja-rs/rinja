use std::env::var_os;
use std::path::PathBuf;

fn main() {
    let Some(toolchain) = var_os("RUSTUP_TOOLCHAIN") else {
        println!("cargo::warning=`RUSTUP_TOOLCHAIN` unset");
        return;
    };

    let toolchain = PathBuf::from(toolchain);
    let toolchain = toolchain.file_name().unwrap_or(toolchain.as_os_str());
    let Some(toolchain) = toolchain.to_str() else {
        println!("cargo::warning=env var `RUSTUP_TOOLCHAIN` is not a UTF-8 string ({toolchain:?})");
        return;
    };

    let (toolchain, _) = toolchain.split_once('-').unwrap_or((toolchain, ""));
    if toolchain == "stable" {
        println!("cargo:rustc-cfg=RUN_UI_TESTS");
    } else {
        println!(
            "cargo::warning=UI tests are ignored on any channel but +stable \
            (running: {toolchain:?})"
        );
    }
}
