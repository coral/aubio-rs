#[cfg(not(feature = "rustdoc"))]
mod source {
    //pub const REPOSITORY: &str = "https://github.com/aubio/aubio";
    //pub const VERSION: &str = "0.4.9";
    pub const REPOSITORY: &str = "https://github.com/katyo/aubio";
    pub const VERSION: &str = "master";

    #[cfg(feature = "with-fftw3")]
    pub mod fftw3 {
        pub const LOCATION: &str = "http://www.fftw.org/fftw-";
        pub const VERSION: &str = "3.3.8";
    }
}

fn main() {
    #[cfg(not(feature = "rustdoc"))]
    {
        use std::{
            env,
            path::Path,
        };

        let src = utils::Source {
            repository: env::var("AUBIO_REPOSITORY")
                .unwrap_or(source::REPOSITORY.into()),
            version: env::var("AUBIO_VERSION")
                .unwrap_or(source::VERSION.into()),
        };

        let out_dir = env::var("OUT_DIR")
            .expect("The OUT_DIR is set by cargo.");

        let out_dir = Path::new(&out_dir);

        #[cfg(feature = "with-fftw3")]
        let fftw3_dir = {
            let src = utils::fftw3::Source {
                location: env::var("FFTW3_LOCATION")
                    .unwrap_or(source::fftw3::LOCATION.into()),
                version: env::var("FFTW3_VERSION")
                    .unwrap_or(source::fftw3::VERSION.into()),
            };

            let src_dir = out_dir.join("fftw3-source")
                .join(&src.version);

            let bld_dir = out_dir.join("fftw3-build")
                .join(&src.version);

            utils::fftw3::fetch_source(&src, &src_dir);

            utils::fftw3::compile_library(&src_dir, &bld_dir);

            bld_dir.join("lib").join("pkgconfig").to_owned()
        };

        let src_dir = out_dir.join("source")
            .join(&src.version);

        let bld_dir = out_dir.join("build")
            .join(&src.version);

        let config = utils::Config {
            #[cfg(feature = "with-fftw3")]
            fftw3_dir: Some(fftw3_dir.to_owned()),

            ..utils::Config::default()
        };

        utils::fetch_source(&src, &src_dir);

        utils::fix_source(&src_dir);

        utils::compile_library(&src_dir, &bld_dir, &config);
    }
}

mod utils {
    use std::{
        env,
        path::{Path, PathBuf},
        process::Command,
    };

    #[derive(Default)]
    pub struct Config {
        pub fftw3_dir: Option<PathBuf>,
    }

    pub struct Source {
        pub repository: String,
        pub version: String,
    }

    pub fn fetch_source(src: &Source, out_dir: &Path) {
        use fetch_unroll::Fetch;

        if !out_dir.is_dir() {
            let src_url = format!("{repo}/archive/{ver}.tar.gz",
                                  repo = src.repository,
                                  ver = src.version);

            eprintln!("Fetch fluidlite from {} to {}",
                      src_url, out_dir.display());

            Fetch::from(src_url).unroll().strip_components(1).to(out_dir)
                .expect("FluidLite sources should be fetched.");
        }
    }

    pub fn fix_source(src_dir: &Path) {
        use std::{
            io::{Read, Write},
            fs::File,
        };

        let scripts = src_dir.join("scripts");
        for script in &["get_waf.sh", "build_mingw", "build_android", "build_emscripten"] {
            let script = scripts.join(script);
            let mut source = String::new();
            File::open(&script).unwrap().read_to_string(&mut source).unwrap();
            if source.starts_with("#! /bin/bash") {
                File::create(&script).unwrap().write(source.replace("#! /bin/bash", "#!/usr/bin/env bash").as_bytes()).unwrap();
            }
        }
    }

    pub fn toolchain_env() -> Vec<(&'static str, String)> {
        let target = env::var("TARGET")
            .expect("The TARGET is set by cargo.");

        let mut env = Vec::new();

        // For cargo: like "CARGO_TARGET_I686_LINUX_ANDROID_CC".  This is really weakly
        // documented; see https://github.com/rust-lang/cargo/issues/5690 and follow
        // links from there.

        // For build.rs in `cc` consumers: like "CC_i686-linux-android". See
        // https://github.com/alexcrichton/cc-rs#external-configuration-via-environment-variables.

        if let Ok(cc) = env::var(format!("CARGO_TARGET_{}_CC", target))
            .or_else(|_| env::var(format!("CC_{}", target))) {
                env.push(("CC", cc));
            }
        if let Ok(ar) = env::var(format!("CARGO_TARGET_{}_AR", target))
            .or_else(|_| env::var(format!("AR_{}", target))) {
                env.push(("AR", ar));
            }
        if let Ok(ld) = env::var(format!("CARGO_TARGET_{}_LINKER", target)) {
            env.push(("LINKER", ld));
        }

        env
    }

    pub fn lib_file<S: AsRef<str>>(name: S, shared: bool) -> String {
        #[cfg(target_os = "windows")]
        {
            format!("{}.{}", name.as_ref(), if shared { "dll" } else { "lib" })
        }

        #[cfg(not(target_os = "windows"))]
        {
            format!("lib{}.{}", name.as_ref(), if shared { "so" } else { "a" })
        }
    }

    pub fn run_command(cmd: &mut Command) {
        use std::{
            process::Output,
            str::from_utf8,
        };

        eprintln!("Run command: {:?}", cmd);

        match cmd.output() {
            Err(error) => {
                panic!("Failed to run command '{:?}' due to: {}", cmd, error);
            },
            Ok(Output { status, stderr, .. }) => {
                if !status.success() {
                    panic!("Command '{:?}' failed with error: {}", cmd,
                           from_utf8(stderr.as_slice())
                           .unwrap_or("<invalud UTF8 string>"));
                }
            }
        }
    }

    pub fn compile_library(src_dir: &Path, out_dir: &Path, config: &Config) {
        let lib_dir = out_dir.join("src");

        let lib_name = String::from("aubio");

        let target = env::var("TARGET")
            .expect("The TARGET is set by cargo.");

        if !lib_dir.join(lib_file(&lib_name, cfg!(feature = "shared"))).is_file() {
            let profile = env::var("PROFILE")
                .expect("The PROFILE is set by cargo.");

            let num_jobs = env::var("NUM_JOBS")
                .expect("The NUM_JOBS is set by cargo.");

            let mut wafopts = String::new();

            if profile == "debug" {
                wafopts.push_str(" --debug");
            }

            let flags = [
                ("docs", false),
                ("tests", false),
                ("examples", false),

                ("double", cfg!(feature = "with-double")),

                ("fftw3f", cfg!(all(feature = "with-fftw3", not(feature = "with-double")))),
                ("fftw3", cfg!(all(feature = "with-fftw3", feature = "with-double"))),

                ("wavread", cfg!(feature = "with-wav")),
                ("wavwrite", cfg!(feature = "with-wav")),

                ("jack", cfg!(feature = "with-jack")),
                ("sndfile", cfg!(feature = "with-sndfile")),
                ("avcodec", cfg!(feature = "with-avcodec")),
                ("samplerate", cfg!(feature = "with-samplerate")),
            ];

            for &(flag, state) in &flags {
                wafopts.push_str(if state { " --enable-" } else { " --disable-" });
                wafopts.push_str(flag);
            }

            wafopts.push_str(" --out=");
            wafopts.push_str(&out_dir.display().to_string());

            let mut pkg_config_path = Vec::new();

            if let Some(dir) = &config.fftw3_dir {
                pkg_config_path.push(dir.display().to_string());
            }

            let mut env_vars = toolchain_env();

            if pkg_config_path.len() > 0 {
                env_vars.push(("PKG_CONFIG_PATH", pkg_config_path.join(":")));
            }

            run_command(Command::new("make")
                        .current_dir(src_dir)
                        .arg(format!("-j{}", num_jobs))
                        .env("WAFOPTS", wafopts)
                        .envs(env_vars));
        }

        println!("cargo:rustc-link-search=native={}", lib_dir.display());

        #[cfg(feature = "shared")]
        println!("cargo:rustc-link-lib={}", lib_name);

        #[cfg(not(feature = "shared"))]
        println!("cargo:rustc-link-lib=static={}", lib_name);

        if target.contains("-apple") {
            println!("cargo:rustc-link-lib=framework=Accelerate");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
        }
    }

    #[cfg(feature = "with-fftw3")]
    pub mod fftw3 {
        use super::{
            toolchain_env,
            run_command,
            lib_file,
        };

        use std::{
            env,
            path::Path,
        };

        pub struct Source {
            pub location: String,
            pub version: String,
        }

        pub fn fetch_source(src: &Source, out_dir: &Path) {
            use fetch_unroll::Fetch;

            if !out_dir.is_dir() {
                let src_url = format!("{pfx}{ver}.tar.gz",
                                      pfx = src.location,
                                      ver = src.version);

                eprintln!("Fetch FFTW3 from {} to {}",
                          src_url, out_dir.display());

                Fetch::from(src_url).unroll().strip_components(1).to(out_dir)
                    .expect("FFTW3 sources should be fetched.");
            }
        }

        pub fn compile_library(src_dir: &Path, out_dir: &Path) {
            use std::process::Command;

            let lib_dir = out_dir.join("lib");

            let lib_name = String::from(if cfg!(feature = "with-double") { "fftw3" } else { "fftw3f" });

            if !lib_dir.join(lib_file(&lib_name, cfg!(feature = "shared"))).is_file() {

                let profile = env::var("PROFILE")
                    .expect("The PROFILE is set by cargo.");

                let num_jobs = env::var("NUM_JOBS")
                    .expect("The NUM_JOBS is set by cargo.");

                let mut configure_args = Vec::new();

                configure_args.push("--with-pic");

                if cfg!(not(feature = "with-double")) {
                    configure_args.push("--enable-single");
                }

                if cfg!(not(feature = "shared-fftw3")) {
                    configure_args.push("--enable-static");
                }

                if cfg!(feature = "shared-fftw3") {
                    configure_args.push("--enable-shared");
                }

                let mut env_vars = toolchain_env();

                if profile == "debug" {
                    env_vars.push(("CFLAGS", "-O0 -g3".into()));
                }

                if profile == "release" {
                    env_vars.push(("CFLAGS", "-O3".into()));
                }

                run_command(Command::new("./configure")
                            .current_dir(&src_dir)
                            .envs(env_vars.clone())
                            .arg(format!("--prefix={}", out_dir.display()))
                            .args(configure_args));

                run_command(Command::new("make")
                            .current_dir(&src_dir)
                            .envs(env_vars)
                            .arg(format!("-j{}", num_jobs))
                            .arg("install"));
            }

            #[cfg(not(feature = "nolink-fftw3"))]
            {
                println!("cargo:rustc-link-search=native={}", lib_dir.display());

                #[cfg(feature = "shared-fftw3")]
                println!("cargo:rustc-link-lib={}", lib_name);

                #[cfg(not(feature = "shared-fftw3"))]
                println!("cargo:rustc-link-lib=static={}", lib_name);
            }
        }
    }
}