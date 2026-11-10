use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const PBOS: [&str; 2] = ["sign_fixture.pbo", "uksf_test_typhoon_hook.pbo"];

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("uksf-sign-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for pbo in PBOS {
        fs::copy(Path::new(FIXTURES).join(pbo), dir.join(pbo)).unwrap();
    }
    dir
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uksf-sign"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn sign_matches_dssignfile_byte_for_byte() {
    let dir = scratch("golden");
    let key = Path::new(FIXTURES).join("uksf_sign_test.biprivatekey");

    let output = run(&dir, &["sign", key.to_str().unwrap(), PBOS[0], PBOS[1]]);

    assert!(output.status.success(), "{output:?}");
    for pbo in PBOS {
        let name = format!("{pbo}.uksf_sign_test.bisign");
        assert_eq!(
            fs::read(dir.join(&name)).unwrap(),
            fs::read(Path::new(FIXTURES).join(&name)).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn verify_accepts_dssignfile_signatures() {
    let key = Path::new(FIXTURES).join("uksf_sign_test.bikey");

    let output = run(
        Path::new(FIXTURES),
        &["verify", key.to_str().unwrap(), PBOS[0], PBOS[1]],
    );

    assert!(output.status.success(), "{output:?}");
}

#[test]
fn created_key_signs_and_verifies() {
    let dir = scratch("roundtrip");

    assert!(
        run(&dir, &["create-key", "uksf_roundtrip"])
            .status
            .success()
    );
    assert!(
        run(&dir, &["sign", "uksf_roundtrip.biprivatekey", PBOS[0]])
            .status
            .success()
    );
    let output = run(&dir, &["verify", "uksf_roundtrip.bikey", PBOS[0]]);

    assert!(output.status.success(), "{output:?}");
}

#[test]
fn verify_rejects_a_changed_pbo() {
    let dir = scratch("tamper");
    for name in [
        format!("{}.uksf_sign_test.bisign", PBOS[0]),
        "uksf_sign_test.bikey".to_string(),
    ] {
        fs::copy(Path::new(FIXTURES).join(&name), dir.join(&name)).unwrap();
    }
    let path = dir.join(PBOS[0]);
    let mut bytes = fs::read(&path).unwrap();
    let marker = bytes.windows(8).position(|w| w == b"hello %1").unwrap();
    bytes[marker] = b'j';
    fs::write(&path, bytes).unwrap();

    let output = run(&dir, &["verify", "uksf_sign_test.bikey", PBOS[0]]);

    assert!(!output.status.success());
}

#[test]
fn verify_rejects_a_key_from_another_authority() {
    let dir = scratch("authority");
    assert!(run(&dir, &["create-key", "uksf_other"]).status.success());
    fs::copy(
        Path::new(FIXTURES).join(format!("{}.uksf_sign_test.bisign", PBOS[0])),
        dir.join(format!("{}.uksf_other.bisign", PBOS[0])),
    )
    .unwrap();

    let output = run(&dir, &["verify", "uksf_other.bikey", PBOS[0]]);

    assert!(!output.status.success());
}

#[test]
fn usage_error_exits_non_zero() {
    let output = run(Path::new(FIXTURES), &["sign"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}
