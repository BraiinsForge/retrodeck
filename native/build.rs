use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=src/vorbis_state.c");
    let target = env::var("TARGET").unwrap();
    let linker_key = format!(
        "CARGO_TARGET_{}_LINKER",
        target.replace('-', "_").to_uppercase()
    );
    println!("cargo:rerun-if-env-changed={linker_key}");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=AR");
    let cc = env::var(&linker_key)
        .or_else(|_| env::var("CC"))
        .unwrap_or_else(|_| "cc".into());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("vorbis_state.o");
    assert!(
        Command::new(&cc)
            .args(["-c", "-fPIC", "src/vorbis_state.c", "-o"])
            .arg(&object)
            .status()
            .expect("compile Vorbis allocation shim")
            .success()
    );
    let ar = env::var("AR").unwrap_or_else(|_| {
        let output = Command::new(&cc)
            .arg("-print-prog-name=ar")
            .output()
            .expect("locate the C compiler's archiver");
        assert!(output.status.success(), "locate the C compiler's archiver");
        String::from_utf8(output.stdout)
            .expect("archiver path is UTF-8")
            .trim()
            .to_owned()
    });
    assert!(
        Command::new(ar)
            .arg("crs")
            .arg(out.join("libvorbis_state.a"))
            .arg(object)
            .status()
            .expect("archive Vorbis allocation shim")
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=vorbis_state");
}
