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
