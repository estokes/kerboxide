use std::{env, path::PathBuf, process::Command};

fn search_pat(base: &str, pat: &str) -> bool {
    let res = Command::new("find")
        .arg(base)
        .arg("-name")
        .arg(pat)
        .output();
    match dbg!(res) {
        Err(_) => false,
        Ok(output) => output.stdout.len() > 0,
    }
}

enum Gssapi {
    Mit,
    Heimdal,
    Apple,
}

fn which() -> Gssapi {
    if cfg!(target_os = "macos") {
        return Gssapi::Apple;
    } else if cfg!(target_os = "windows") {
        panic!("use SSPI on windows")
    } else if cfg!(target_family = "unix") {
        let ldpath = env::var("LD_LIBRARY_PATH").unwrap_or(String::new());
        let paths = vec!["/lib", "/lib64", "/usr/lib", "/usr/lib64"];
        let krb5_path = Command::new("krb5-config")
            .arg("--prefix")
            .arg("gssapi")
            .output()
            .map(|o| o.stdout)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        let krb5_path = krb5_path.as_ref().map(|s| s.trim());
        for path in krb5_path.into_iter().chain(ldpath.split(':')).chain(paths) {
            if !path.is_empty() {
                if search_pat(path, "libgssapi_krb5.so*") {
                    return Gssapi::Mit;
                }
                if search_pat(path, "libgssapi.so*") {
                    return Gssapi::Heimdal;
                }
            }
        }
        panic!("no gssapi implementation found, install mit kerberos or heimdal");
    } else {
        panic!("libgssapi isn't ported to this platform yet")
    }
}

fn main() {
    let imp = which();
    match imp {
        Gssapi::Mit => println!("cargo:rustc-link-lib=gssapi_krb5"),
        Gssapi::Heimdal => println!("cargo:rustc-link-lib=gssapi"),
        Gssapi::Apple => println!("cargo:rustc-link-lib=framework=GSS"),
    }
    let builder = bindgen::Builder::default();
    let nix_cflags = env::var("NIX_CFLAGS_COMPILE");
    let builder = match imp {
        Gssapi::Mit | Gssapi::Heimdal => match nix_cflags {
            Err(_) => builder,
            Ok(flags) => builder.clang_args(flags.split(" ")),
        },
        Gssapi::Apple => {
            let sdk_path = Command::new("xcrun")
                .arg("--show-sdk-path")
                .output()
                .map(|o| o.stdout)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .expect("failed to run `xcrun --show-sdk-path'");
            let sdk_path = sdk_path.trim();
            builder.clang_arg(format!("-F{}/System/Library/Frameworks", sdk_path))
        }
    };
    let bindings = builder
        .allowlist_type("(OM_.+|gss_.+)")
        .allowlist_var("_?GSS_.+|gss_.+")
        .allowlist_function("gss_.*")
        .header(match imp {
            Gssapi::Mit => "src/wrapper_mit.h",
            Gssapi::Heimdal => "src/wrapper_heimdal.h",
            Gssapi::Apple => "src/wrapper_apple.h",
        })
        .generate()
        .expect("failed to generate gssapi bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings")
}
