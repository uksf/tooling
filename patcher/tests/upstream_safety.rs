use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
struct Sandbox {
    root: PathBuf,
    original: Vec<u8>,
}
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "patcher-upstream-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("mod/addons")).unwrap();
        fs::create_dir_all(root.join("patches")).unwrap();
        let mut original = b"a\0".to_vec();
        for n in [0u32, 3, 0, 0, 3] {
            original.extend_from_slice(&n.to_le_bytes());
        }
        original.extend_from_slice(&[0; 21]);
        original.extend_from_slice(b"abc");
        let digest = Sha1::digest(&original);
        original.push(0);
        original.extend_from_slice(&digest);
        fs::write(root.join("mod/addons/x.pbo"), &original).unwrap();
        let manifest = serde_json::json!({"format":1,"pbo":"x.pbo","pristineSha256":hash(&original),"assets":[{"path":"a","sha256":hash(b"abc"),"ops":[{"op":"text-replace","find":"a","replace":"x"}]}]});
        fs::write(
            root.join("patches/x.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        Self { root, original }
    }
    fn run(&self, command: &str, extra: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_uksf-patcher"))
            .arg(command)
            .arg("--addons")
            .arg(self.root.join("mod/addons"))
            .arg("--patches")
            .arg(self.root.join("patches"))
            .arg("--state")
            .arg(self.root.join("state"))
            .args(extra)
            .output()
            .unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn upstream_change_fails_apply_dry_run_and_verify_without_writes() {
    let b = Sandbox::new();
    let mut upstream = b.original.clone();
    upstream[43] ^= 1;
    let pbo = b.root.join("mod/addons/x.pbo");
    fs::write(&pbo, &upstream).unwrap();
    for (command, extra) in [
        ("apply", vec![]),
        ("apply", vec!["--dry-run"]),
        ("verify", vec![]),
    ] {
        let result = b.run(command, &extra);
        assert_eq!(result.status.code(), Some(1));
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains(&format!("expected sha256 {}", hash(&b.original))));
        assert!(error.contains(&format!("actual {}", hash(&upstream))));
        assert_eq!(fs::read(&pbo).unwrap(), upstream);
    }
    assert!(!b.root.join("state").exists());
}
#[test]
fn hash_matching_corrupt_backup_still_requires_valid_trailer() {
    let b = Sandbox::new();
    assert!(b.run("apply", &[]).status.success());
    let backup = b.root.join("state/x.pbo.pristine");
    let mut damaged = b.original.clone();
    damaged[43] ^= 1;
    fs::write(&backup, &damaged).unwrap();
    let manifest_path = b.root.join("patches/x.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["pristineSha256"] = hash(&damaged).into();
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let state_path = b.root.join("state/x.pbo.state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
    state["PristineSha256"] = hash(&damaged).into();
    fs::write(state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    let result = b.run("verify", &[]);
    assert_eq!(result.status.code(), Some(1));
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(error.contains("SHA-1 trailer"), "{error}");
    assert!(error.contains("x.pbo.pristine"));
}
