use std::{
    env,
    path::{Path, PathBuf},
};

#[cfg(feature = "snappy")]
const SNAPPY_VERSION: &str = "1.2.2";

const LEVELDB_VERSION: &str = "1.22";
/// Directory name within `$OUT_DIR` where the static libraries should be built.
const LIBDIR: &str = "lib";

#[cfg(feature = "snappy")]
fn build_snappy() -> PathBuf {
    use std::process::Command;
    let outdir = env::var("OUT_DIR").unwrap();

    println!("[snappy] Checkout");
    let snappy_dir = Path::new(&outdir).join(format!("snappy-{SNAPPY_VERSION}"));
    if !snappy_dir.join("CMakeLists.txt").exists() {
        let output = Command::new("git")
            .arg("clone")
            .arg("--branch")
            .arg(SNAPPY_VERSION)
            .arg("https://github.com/google/snappy.git")
            .arg(snappy_dir.clone())
            .arg("--recurse-submodules")
            .output()
            .expect("failed to execute git");
        if !output.status.success() {
            panic!("failed to checkout snappy: {}", String::from_utf8_lossy(&output.stderr));
        }
    }

    println!("[snappy] Building");

    let libdir = Path::new(&outdir).join(LIBDIR);

    env::set_var("NUM_JOBS", num_cpus::get().to_string());
    let dest_prefix = cmake::Config::new(snappy_dir)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("SNAPPY_BUILD_TESTS", "OFF")
        .define("SNAPPY_BUILD_BENCHMARKS", "OFF")
        .define("HAVE_LIBZ", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", &libdir)
        .build();

    assert_eq!(
        dest_prefix.join(LIBDIR),
        libdir,
        "CMake should build Snappy in provided LIBDIR"
    );
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=static=snappy");

    dest_prefix
}

fn build_leveldb(snappy_prefix: Option<PathBuf>) {
    println!("[leveldb] Building");

    let outdir = env::var("OUT_DIR").unwrap();
    let libdir = Path::new(&outdir).join(LIBDIR);

    env::set_var("NUM_JOBS", num_cpus::get().to_string());
    let mut config =
        cmake::Config::new(Path::new("deps").join(format!("leveldb-{LEVELDB_VERSION}")));
    config
        .define("LEVELDB_BUILD_TESTS", "OFF")
        .define("LEVELDB_BUILD_BENCHMARKS", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", &libdir);
    if let Some(snappy_prefix) = snappy_prefix {
        #[cfg(target_env = "msvc")]
        let ldflags = format!("/LIBPATH:{}", snappy_prefix.join(LIBDIR).display());
        #[cfg(not(target_env = "msvc"))]
        let ldflags = format!("-L{}", snappy_prefix.join(LIBDIR).display());

        env::set_var("LDFLAGS", ldflags);

        config
            .define("HAVE_SNAPPY", "ON")
            .cflag(format!("-I{}", snappy_prefix.join("include").display()))
            .cxxflag(format!("-I{}", snappy_prefix.join("include").display()));
    } else {
        config.define("HAVE_SNAPPY", "OFF");
    }
    let dest_prefix = config.build();

    assert_eq!(
        dest_prefix.join(LIBDIR),
        libdir,
        "CMake should build LevelDB in provided LIBDIR"
    );
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=static=leveldb");
}

fn main() {
    println!("[build] Started");

    // If we have the appropriate feature, then we build snappy.
    #[cfg(feature = "snappy")]
    let snappy_prefix = Some(build_snappy());
    #[cfg(not(feature = "snappy"))]
    let snappy_prefix: Option<PathBuf> = None;

    // Build LevelDB
    build_leveldb(snappy_prefix);

    // Link to the standard C++ library
    let target = env::var("TARGET").unwrap();
    if target.contains("apple") || target.contains("freebsd") {
        println!("cargo:rustc-link-lib=c++");
    } else if target.contains("gnu") || target.contains("netbsd") || target.contains("openbsd") {
        println!("cargo:rustc-link-lib=stdc++");
    } else if target.contains("musl") {
        // We want to link to libstdc++ *statically*. This requires that the user passes the right
        // search path to rustc via `-Lstatic=/path/to/libstdc++`.
        println!("cargo:rustc-link-lib=static=stdc++");
    }

    println!("[build] Finished");
}
