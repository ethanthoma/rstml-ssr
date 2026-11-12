use std::{env, fs, process::Command};

#[test]
fn renamed_dependency_renders_without_a_canonical_crate_import() {
    let directory = env::temp_dir().join(format!("rstml-ssr-consumer-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    fs::create_dir(directory.join("src")).unwrap();
    fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[package]\nname = \"renamed-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nrenamed = {{ package = \"rstml-ssr\", path = {:?} }}\n",
            env!("CARGO_MANIFEST_DIR")
        ),
    ).unwrap();
    let lockfile = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .take(4)
        .map(|directory| directory.join("Cargo.lock"))
        .find(|path| path.is_file())
        .expect("workspace or packaged crate lockfile");
    fs::copy(lockfile, directory.join("Cargo.lock")).unwrap();
    fs::write(
        directory.join("src/main.rs"),
        include_str!("fixtures/renamed.rs"),
    )
    .unwrap();
    let result = Command::new(env!("CARGO"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["run", "--quiet", "--offline", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", directory.join("target"))
        .output();
    fs::remove_dir_all(&directory).unwrap();
    let output = result.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
