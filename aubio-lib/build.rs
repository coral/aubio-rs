#[cfg(not(feature = "rustdoc"))]
mod source {
    pub const URL: &str = "https://github.com/aubio/aubio/archive/{version}.tar.gz";
    pub const VERSION: &str = "0.4.9";

    pub mod waf {
        pub const URL: &str = "https://github.com/kukuen/{package}/releases/download/{version}/waf-light-{version}.tar.gz";
        pub const VERSION: &str = "2.0.20-msvc-fix";
    }

    #[cfg(feature = "with-fftw3")]
    pub mod fftw3 {
        pub const URL: &str = "http://www.fftw.org/{package}-{version}.tar.gz";
        pub const VERSION: &str = "3.3.8";
    }
}

fn main() {
    #[cfg(not(feature = "rustdoc"))]
    {
        use std::{env, path::Path};

        let aubio_src = utils::Source::new(
            "aubio",
            env::var("AUBIO_VERSION").unwrap_or_else(|_| source::VERSION.into()),
            env::var("AUBIO_URL").unwrap_or_else(|_| source::URL.into()),
        );

        let waf_src = utils::Source::new(
            "waf",
            env::var("WAF_VERSION").unwrap_or_else(|_| source::waf::VERSION.into()),
            env::var("WAF_URL").unwrap_or_else(|_| source::waf::URL.into()),
        );

        let out_dir = env::var("OUT_DIR").expect("The OUT_DIR is set by cargo.");

        let out_dir = Path::new(&out_dir);

        #[cfg(feature = "with-fftw3")]
        let fftw3_dir = {
            let src = utils::Source::new(
                "fftw",
                env::var("FFTW3_VERSION").unwrap_or(source::fftw3::VERSION.into()),
                env::var("FFTW3_URL").unwrap_or(source::fftw3::URL.into()),
            );

            let src_dir = out_dir.join("fftw3-source").join(&src.version);

            let bld_dir = out_dir.join("fftw3-build").join(&src.version);

            utils::fftw3::fetch_source(&src, &src_dir);

            utils::fftw3::compile_library(&src_dir, &bld_dir);

            bld_dir.join("lib").join("pkgconfig").to_owned()
        };

        let src_dir = out_dir.join("source").join(&aubio_src.version);

        let bld_dir = out_dir.join("build").join(&aubio_src.version);

        let config = utils::Config {
            #[cfg(feature = "with-fftw3")]
            fftw3_dir: Some(fftw3_dir.to_owned()),

            ..utils::Config::default()
        };

        if !src_dir.is_dir() {
            utils::fetch_source("aubio", &aubio_src, &src_dir);
            utils::fetch_source("waf", &waf_src, &src_dir);
        }

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
        pub package: String,
        pub version: String,
        pub url: String,
    }

    impl Source {
        pub fn new(
            package: impl Into<String>,
            version: impl Into<String>,
            url: impl Into<String>,
        ) -> Self {
            Self {
                package: package.into(),
                version: version.into(),
                url: url.into(),
            }
        }

        pub fn url(&self) -> String {
            self.url
                .replace("{package}", &self.package)
                .replace("{version}", &self.version)
        }
    }

    pub fn fetch_source(package: &str, src: &Source, out_dir: &Path) {
        use fetch_unroll::Fetch;

        let src_url = src.url();

        eprintln!(
            "Fetch {} from {} to {}",
            package,
            src_url,
            out_dir.display()
        );

        Fetch::from(src_url)
            .unroll()
            .cleanup_dest_dir(false)
            .strip_components(1)
            .to(out_dir)
            .unwrap_or_else(|_| panic!("{} sources should be fetched.", package));
    }

    pub fn fix_source(src_dir: &Path) {
        use std::{
            fs::File,
            io::{Read, Write},
        };

        let scripts = src_dir.join("scripts");
        for script in &[
            "get_waf.sh",
            "build_mingw",
            "build_android",
            "build_emscripten",
        ] {
            let script = scripts.join(script);
            let mut source = String::new();
            File::open(&script)
                .unwrap()
                .read_to_string(&mut source)
                .unwrap();
            if source.starts_with("#! /bin/bash") {
                File::create(&script)
                    .unwrap()
                    .write_all(
                        source
                            .replace("#! /bin/bash", "#!/usr/bin/env bash")
                            .as_bytes(),
                    )
                    .unwrap();
            }
        }

        {
            // disable gensyms
            let script = src_dir.join("wscript");
            let mut source = String::new();
            File::open(&script)
                .unwrap()
                .read_to_string(&mut source)
                .unwrap();
            File::create(&script)
                .unwrap()
                .write_all(
                    source
                        .replace("    ctx.load('waf_gensyms', tooldir='.')", "")
                        .as_bytes(),
                )
                .unwrap();
        }
    }

    pub fn toolchain_env() -> Vec<(&'static str, String)> {
        let target = env::var("TARGET").expect("The TARGET is set by cargo.");

        let mut env = Vec::new();

        // For cargo: like "CARGO_TARGET_I686_LINUX_ANDROID_CC".  This is really weakly
        // documented; see https://github.com/rust-lang/cargo/issues/5690 and follow
        // links from there.

        // For build.rs in `cc` consumers: like "CC_i686-linux-android". See
        // https://github.com/alexcrichton/cc-rs#external-configuration-via-environment-variables.

        if let Ok(cc) = env::var(format!("CARGO_TARGET_{}_CC", target))
            .or_else(|_| env::var(format!("CC_{}", target)))
        {
            env.push(("CC", cc));
        }
        if let Ok(ar) = env::var(format!("CARGO_TARGET_{}_AR", target))
            .or_else(|_| env::var(format!("AR_{}", target)))
        {
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

    pub fn run_command(cmd: &mut Command) -> String {
        use std::{process::Output, str::from_utf8};

        eprintln!("Run command: {:?}", cmd);

        match cmd.output() {
            Err(error) => {
                panic!("Failed to run command '{:?}' due to: {}", cmd, error);
            }
            Ok(Output {
                status,
                stdout,
                stderr,
                ..
            }) => {
                if !status.success() {
                    panic!(
                        "Command '{:?}' failed (stdout: {}) (stderr: {})",
                        cmd,
                        from_utf8(stdout.as_slice()).unwrap_or("<invalud UTF8 string>"),
                        from_utf8(stderr.as_slice()).unwrap_or("<invalud UTF8 string>")
                    );
                }

                String::from_utf8(stdout).unwrap_or_else(|err| {
                    panic!("Invalid unicode output of command '{:?}' ({:?})", cmd, err)
                })
            }
        }
    }

    pub fn compile_library(src_dir: &Path, out_dir: &Path, config: &Config) {
        let lib_dir = out_dir.join("lib");

        let lib_name = String::from("aubio");

        let target = env::var("TARGET").expect("The TARGET is set by cargo.");

        if !lib_dir
            .join(lib_file(&lib_name, cfg!(feature = "shared")))
            .is_file()
        {
            let profile = env::var("PROFILE").expect("The PROFILE is set by cargo.");
            let target_arch = env::var("CARGO_CFG_TARGET_ARCH")
                .expect("The CARGO_CFG_TARGET_ARCH is set by cargo.");
            let target_os =
                env::var("CARGO_CFG_TARGET_OS").expect("The CARGO_CFG_TARGET_OS is set by cargo.");
            let target_env = env::var("CARGO_CFG_TARGET_ENV")
                .expect("The CARGO_CFG_TARGET_ENV is set by cargo.");
            let num_jobs = env::var("NUM_JOBS").expect("The NUM_JOBS is set by cargo.");

            let mut wafargs = Vec::<String>::new();

            if target_os == "windows" {
                wafargs.push(format!(
                    "--check-c-compiler={}",
                    if target_env == "msvc" { "msvc" } else { "gcc" }
                ));
            }

            if let Some(target_platform) = match target_os.as_ref() {
                // WAF failed to find gcc when target platform is set
                "windows" => Some(if target_arch.ends_with("64") {
                    "win64"
                } else {
                    "win32"
                }),
                "macos" => Some("darwin"),
                "ios" => Some(
                    if target_arch.starts_with("arm") || target_arch.starts_with("aarch") {
                        "ios"
                    } else {
                        "iosimulator"
                    },
                ),
                os => Some(os),
            } {
                wafargs.push(format!("--with-target-platform={}", target_platform));
            }

            wafargs.push("--verbose".into());

            wafargs.push("--build-type".into());
            wafargs.push(profile);

            wafargs.push("--jobs".into());
            wafargs.push(num_jobs);

            let flags = [
                ("docs", false),
                ("tests", false),
                ("examples", false),
                ("double", cfg!(feature = "with-double")),
                (
                    "fftw3f",
                    cfg!(all(feature = "with-fftw3", not(feature = "with-double"))),
                ),
                (
                    "fftw3",
                    cfg!(all(feature = "with-fftw3", feature = "with-double")),
                ),
                ("wavread", cfg!(feature = "with-wav")),
                ("wavwrite", cfg!(feature = "with-wav")),
                ("jack", cfg!(feature = "with-jack")),
                ("sndfile", cfg!(feature = "with-sndfile")),
                ("avcodec", cfg!(feature = "with-avcodec")),
                ("samplerate", cfg!(feature = "with-samplerate")),
            ];

            for &(flag, state) in &flags {
                wafargs.push(format!(
                    "--{}-{}",
                    if state { "enable" } else { "disable" },
                    flag
                ));
            }

            wafargs.push("--out".into());
            wafargs.push(out_dir.display().to_string());
            wafargs.push("--prefix".into());
            wafargs.push(out_dir.display().to_string());

            let mut pkg_config_path = Vec::new();

            if let Some(dir) = &config.fftw3_dir {
                pkg_config_path.push(dir.display().to_string());
            }

            let mut env_vars = toolchain_env();

            if !pkg_config_path.is_empty() {
                env_vars.push(("PKG_CONFIG_PATH", pkg_config_path.join(":")));
            }

            let python_path = find_python().expect("Cannot find appropriate python interpreter");

            for task in &["configure", "build", "install"] {
                let _ = run_command(
                    Command::new(&python_path)
                        .envs(env_vars.clone())
                        .current_dir(src_dir)
                        .arg("waf-light")
                        .args(&wafargs)
                        .arg(task),
                );
            }
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

    fn find_python() -> Result<PathBuf, String> {
        use which::which;

        which("python3")
            .or_else(|_| which("python"))
            .map_err(|err| err.to_string())
            .and_then(|path| {
                let out = run_command(Command::new(&path).arg("-V"));
                if out.starts_with("Python 3.") {
                    Ok(path)
                } else {
                    Err("Python 3.x required".into())
                }
            })
    }

    #[cfg(feature = "with-fftw3")]
    pub mod fftw3 {
        use super::{lib_file, Source};

        use std::path::Path;

        pub fn fetch_source(src: &Source, out_dir: &Path) {
            use fetch_unroll::Fetch;

            if !out_dir.is_dir() {
                let src_url = src.url();

                eprintln!("Fetch FFTW3 from {} to {}", src_url, out_dir.display());

                Fetch::from(src_url)
                    .unroll()
                    .strip_components(1)
                    .to(out_dir)
                    .expect("FFTW3 sources should be fetched.");
            }
        }

        pub fn compile_library(src_dir: &Path, out_dir: &Path) {
            use cmake::Config;

            let lib_dir = out_dir.join("lib");

            let lib_name = String::from(if cfg!(feature = "with-double") {
                "fftw3"
            } else {
                "fftw3f"
            });

            if !lib_dir
                .join(lib_file(&lib_name, cfg!(feature = "shared")))
                .is_file()
            {
                use std::fs::{create_dir_all, rename};

                create_dir_all(out_dir).unwrap();

                fn bool_flag(flag: bool) -> &'static str {
                    if flag {
                        "ON"
                    } else {
                        "OFF"
                    }
                }

                let _library = Config::new(src_dir)
                    .define(
                        "BUILD_SHARED_LIBS",
                        bool_flag(cfg!(feature = "shared-fftw3")),
                    )
                    .define("BUILD_TESTS", bool_flag(false))
                    .define("ENABLE_FLOAT", bool_flag(!cfg!(feature = "with-double")))
                    .define("DISABLE_FORTRAN", bool_flag(true))
                    .define("ENABLE_SSE", bool_flag(cfg!(target_feature = "sse")))
                    .define("ENABLE_SSE2", bool_flag(cfg!(target_feature = "sse2")))
                    .define("ENABLE_AVX", bool_flag(cfg!(target_feature = "avx")))
                    .define("ENABLE_AVX2", bool_flag(cfg!(target_feature = "avx2")))
                    .define("CMAKE_INSTALL_LIBDIR", "lib")
                    .define("CMAKE_C_COMPILER_WORKS", bool_flag(true))
                    .define("CMAKE_CXX_COMPILER_WORKS", bool_flag(true))
                    .always_configure(true)
                    .very_verbose(true)
                    .out_dir(out_dir)
                    .build();

                {
                    // fix misnamed pkg configs
                    let pc_dir = out_dir.join("lib").join("pkgconfig");

                    #[cfg(not(feature = "with-double"))]
                    let _ = rename(pc_dir.join("fftwf.pc"), pc_dir.join("fftw3f.pc"));

                    #[cfg(feature = "with-double")]
                    let _ = rename(pc_dir.join("fftw.pc"), pc_dir.join("fftw3.pc"));
                }
            }
        }
    }
}
