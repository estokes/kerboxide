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
        let ldpath = env::var("LD_LIBRARY_PATH").unwrap_or_default();
        let paths = vec!["/lib", "/lib64", "/usr/lib", "/usr/lib64"];
        for path in ldpath.split(':').chain(paths) {
            if search_pat(path, "libgssapi_krb5.so*") {
                return Gssapi::Mit;
            }
            if search_pat(path, "libgssapi.so*") {
                return Gssapi::Heimdal;
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
    let builder = match imp {
        Gssapi::Mit | Gssapi::Heimdal => builder,
        Gssapi::Apple =>
            builder.clang_arg("-F/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks")
    };
    let bindings = builder
        .whitelist_type("(OM_.+|gss_.+)")
        .whitelist_var("_?GSS_.+|gss_.+")
        .whitelist_function("gss_.*|__ApplePrivate.*")
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
