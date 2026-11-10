use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
struct Fixture {
    root: PathBuf,
    original: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "patcher-errors-{}-{}",
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
        Self { root, original }
    }
    fn manifest(&self, assets: serde_json::Value) {
        let value = serde_json::json!({"format":1,"pbo":"x.pbo","pristineSha256":hash(&self.original),"assets":assets});
        fs::write(
            self.root.join("patches/x.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    fn asset(&self) -> serde_json::Value {
        serde_json::json!({"path":"a","sha256":hash(b"abc"),"ops":[{"op":"text-replace","find":"a","replace":"z"}]})
    }
    fn run(&self, command: &str, state: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_uksf-patcher"))
            .arg(command)
            .arg("--addons")
            .arg(self.root.join("mod/addons"))
            .arg("--patches")
            .arg(self.root.join("patches"))
            .arg("--state")
            .arg(self.root.join(state))
            .output()
            .unwrap()
    }
    fn assert_rejected(&self, needle: &str) {
        let result = self.run("apply", "state");
        assert_eq!(result.status.code(), Some(1));
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains(needle), "{error}");
        assert_eq!(
            fs::read(self.root.join("mod/addons/x.pbo")).unwrap(),
            self.original
        );
        assert!(!self.root.join("state").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn rejects_duplicate_assets_invalid_copies_unknown_ops_and_match_counts() {
    let f = Fixture::new();
    let asset = f.asset();
    f.manifest(serde_json::json!([asset, asset]));
    f.assert_rejected("listed twice");
    let mut asset = f.asset();
    asset["copyOf"] = "a".into();
    f.manifest(serde_json::json!([asset]));
    f.assert_rejected("already exists");
    let mut asset = f.asset();
    asset["path"] = "bad//name".into();
    asset["copyOf"] = "a".into();
    f.manifest(serde_json::json!([asset]));
    f.assert_rejected("not a valid PBO entry name");
    let mut asset = f.asset();
    asset["ops"][0]["op"] = "invented".into();
    f.manifest(serde_json::json!([asset]));
    f.assert_rejected("unknown op 'invented'");
    let mut asset = f.asset();
    asset["ops"][0]["count"] = 2.into();
    f.manifest(serde_json::json!([asset]));
    f.assert_rejected("expected 2 match(es) of \"a\", found 1");
}

#[test]
fn checks_all_asset_pins_before_running_any_operation() {
    let f = Fixture::new();
    let mut first = f.asset();
    first["ops"][0]["op"] = "invented".into();
    let mut second = f.asset();
    second["path"] = "copy".into();
    second["copyOf"] = "a".into();
    second["sha256"] = "0".repeat(64).into();
    f.manifest(serde_json::json!([first, second]));
    f.assert_rejected("does not match its pin");
}

#[test]
fn comments_parse_and_state_inside_mod_is_rejected() {
    let f = Fixture::new();
    f.manifest(serde_json::json!([f.asset()]));
    let path = f.root.join("patches/x.json");
    let json = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("/* before */ {json} // after\n")).unwrap();
    let result = f.run("apply", "mod/state");
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("--state must be outside"));
    assert!(f.run("apply", "state").status.success());
    assert!(f.run("verify", "state").status.success());
}

#[test]
fn corrupt_backup_fails_closed() {
    let f = Fixture::new();
    f.manifest(serde_json::json!([f.asset()]));
    assert!(f.run("apply", "state").status.success());
    let patched = fs::read(f.root.join("mod/addons/x.pbo")).unwrap();
    fs::write(f.root.join("state/x.pbo.pristine"), b"corrupt").unwrap();
    let result = f.run("verify", "state");
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing or corrupt"));
    assert_eq!(fs::read(f.root.join("mod/addons/x.pbo")).unwrap(), patched);
}
