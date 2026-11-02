// The Windows-only WMA decoder is a separate killable process. No COM pointers
// or unsafe Windows FFI cross into the Rust media pipeline.
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../../tools/native-wma/helper.cpp");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
        Ok("x86_64"),
        "WMA helper currently supports Windows x64 only"
    );
    let vswhere = PathBuf::from(env::var_os("ProgramFiles(x86)").expect("ProgramFiles(x86)"))
        .join("Microsoft Visual Studio/Installer/vswhere.exe");
    let result = Command::new(vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .expect("Find installed MSVC for the embedded Windows WMA decoder");
    assert!(result.status.success(), "MSVC discovery failed");
    let install = String::from_utf8(result.stdout).expect("Visual Studio path encoding");
    let setup = PathBuf::from(install.trim()).join("VC/Auxiliary/Build/vcvars64.bat");
    assert!(
        setup.is_file(),
        "Install MSVC x64 C++ build tools to build the Windows WMA decoder"
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../tools/native-wma/helper.cpp");
    let script = out.join("build-wma.cmd");
    // Paths come only from the installed toolchain and Cargo, never media input.
    for path in [&setup, &source, &out] {
        assert!(
            !path.to_string_lossy().contains(['"', '%', '\r', '\n']),
            "Unsupported build path"
        );
    }
    fs::write(&script, format!(
        "@call \"{}\" >nul\r\n@if errorlevel 1 exit /b 1\r\n@cl /nologo /EHsc /std:c++17 /W4 /WX /O2 /MT \"{}\" /Fe:diaxus-wma.exe mfplat.lib mfreadwrite.lib mfuuid.lib ole32.lib\r\n",
        setup.display(), source.display(),
    )).unwrap();
    let status = Command::new("cmd.exe")
        .arg("/c")
        .arg(&script)
        .current_dir(&out)
        .status()
        .expect("Compile embedded WMA decoder");
    assert!(status.success(), "Embedded WMA decoder build failed");
    println!(
        "cargo:rustc-env=DIAXUS_WMA_HELPER_BUILD_PATH={}",
        out.join("diaxus-wma.exe").display()
    );
}
