use sha1::{Digest, Sha1};
use std::{fs, process::Command};

#[test]
fn hashes_the_file_bytes() {
    let path = std::env::temp_dir().join(format!("uksf-patcher-hash-{}", std::process::id()));
    fs::write(&path, b"abc").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_uksf-patcher"))
        .arg("hash")
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().trim(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn p3d_reports_model_summary() {
    let model = include_bytes!("data/plug.p3d");
    let mut bytes = b"plug.p3d\0".to_vec();
    for word in [0u32, model.len() as u32, 0, 0, model.len() as u32] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&[0; 21]);
    bytes.extend_from_slice(model);
    let trailer = Sha1::digest(&bytes);
    bytes.push(0);
    bytes.extend_from_slice(&trailer);
    let path = std::env::temp_dir().join(format!("uksf-patcher-model-{}.pbo", std::process::id()));
    fs::write(&path, bytes).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_uksf-patcher"))
        .args(["p3d", path.to_str().unwrap(), "plug.p3d"])
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(result.status.success());
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("ODOL v75, 7 LODs, 53669 bytes, sha256 78b66729839290b52388c5bb14d6446b01a9a8417978488d4dd43751d9d272de, re-serialises identically: True"));
    assert!(output.contains("skeleton '': 0 bones"));
    assert!(output.contains("lod geometry (9.99999983E+12): 8 vertices, 12 faces, 1 sections, 1 selections (1 components)"));
}

#[test]
fn missing_arguments_print_usage_and_exit_two() {
    let result = Command::new(env!("CARGO_BIN_EXE_uksf-patcher"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("uksf-patcher apply")
    );
}
