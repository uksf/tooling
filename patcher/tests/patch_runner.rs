use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use uksf_patcher::pbo::Pbo;

static NEXT: AtomicUsize = AtomicUsize::new(0);
fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn fixture() -> Vec<u8> {
    let mut result = Vec::new();
    for (name, packing, size, data) in [
        (b"file.txt".as_slice(), 0u32, 7u32, b"a=1; 1;".as_slice()),
        (b"decoy".as_slice(), 0x43707273, 10, b"bad".as_slice()),
    ] {
        result.extend_from_slice(name);
        result.push(0);
        for word in [packing, size, 0, 123, data.len() as u32] {
            result.extend_from_slice(&word.to_le_bytes());
        }
    }
    result.extend_from_slice(&[0; 21]);
    result.extend_from_slice(b"a=1; 1;bad");
    let digest = Sha1::digest(&result);
    result.push(0);
    result.extend_from_slice(&digest);
    result
}
struct Boxed {
    root: PathBuf,
    pristine: Vec<u8>,
}
impl Boxed {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "patcher-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("mod/addons")).unwrap();
        fs::create_dir_all(root.join("patches")).unwrap();
        let pristine = fixture();
        fs::write(root.join("mod/addons/x.pbo"), &pristine).unwrap();
        Self { root, pristine }
    }
    fn manifest(&self, assets: serde_json::Value) {
        fs::write(self.root.join("patches/x.json"), serde_json::to_vec(&serde_json::json!({"format":1,"pbo":"x.pbo","pristineSha256":hash(&self.pristine),"assets":assets})).unwrap()).unwrap();
    }
    fn asset(&self, replacement: &str) -> serde_json::Value {
        serde_json::json!([{"path":"file.txt","sha256":hash(b"a=1; 1;"),"ops":[{"op":"text-replace","find":"1","replace":replacement,"count":2}]}])
    }
    fn run(&self, command: &str, extra: &[&str]) -> Output {
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
    fn pbo(&self) -> Vec<u8> {
        fs::read(self.root.join("mod/addons/x.pbo")).unwrap()
    }
}
impl Drop for Boxed {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn applies_verifies_rebuilds_and_restores() {
    let b = Boxed::new();
    b.manifest(b.asset("2"));
    let dry = b.run("apply", &["--dry-run"]);
    assert_eq!(dry.status.code(), Some(0));
    assert!(out(&dry).contains("would patch"));
    assert_eq!(b.pbo(), b.pristine);
    assert!(!b.root.join("state").exists());
    let before = b.run("verify", &[]);
    assert_eq!(before.status.code(), Some(1));
    assert!(out(&before).contains("NOT APPLIED"));
    let first = b.run("apply", &[]);
    assert_eq!(first.status.code(), Some(0), "{}", err(&first));
    let pbo = Pbo::parse(b.pbo()).unwrap();
    assert_eq!(pbo.data(0).unwrap(), b"a=2; 2;");
    assert!(pbo.trailer_valid());
    assert_eq!(
        &pbo.bytes[pbo.entries[1].offset..pbo.entries[1].offset + 3],
        b"bad"
    );
    assert_eq!(
        fs::read(b.root.join("state/x.pbo.pristine")).unwrap(),
        b.pristine
    );
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(b.root.join("state/x.pbo.state.json")).unwrap()).unwrap();
    assert_eq!(state["PatchedSha256"], hash(&b.pbo()));
    assert!(state["PendingPatchedSha256"].is_null());
    let modified = fs::metadata(b.root.join("mod/addons/x.pbo"))
        .unwrap()
        .modified()
        .unwrap();
    let second = b.run("apply", &[]);
    assert_eq!(second.status.code(), Some(0));
    assert!(out(&second).contains("up to date"));
    assert_eq!(
        modified,
        fs::metadata(b.root.join("mod/addons/x.pbo"))
            .unwrap()
            .modified()
            .unwrap()
    );
    assert_eq!(b.run("verify", &[]).status.code(), Some(0));
    b.manifest(b.asset("3"));
    let changed = b.run("apply", &[]);
    assert_eq!(changed.status.code(), Some(0), "{}", err(&changed));
    assert!(out(&changed).contains("patch set changed"));
    assert_eq!(Pbo::parse(b.pbo()).unwrap().data(0).unwrap(), b"a=3; 3;");
    fs::remove_file(b.root.join("patches/x.json")).unwrap();
    assert_eq!(b.run("apply", &[]).status.code(), Some(0));
    assert_eq!(b.pbo(), b.pristine);
    assert_eq!(fs::read_dir(b.root.join("state")).unwrap().count(), 0);
}

#[test]
fn rejects_bad_pins_and_unported_ops_without_writes() {
    let b = Boxed::new();
    let mut asset = b.asset("2");
    asset[0]["sha256"] = "0".repeat(64).into();
    b.manifest(asset);
    let bad = b.run("apply", &[]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(err(&bad).contains("does not match its pin"));
    assert_eq!(b.pbo(), b.pristine);
    assert!(!b.root.join("state").exists());
    let mut asset = b.asset("2");
    asset[0]["ops"][0]["op"] = "add-mesh".into();
    b.manifest(asset);
    let bad = b.run("apply", &[]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(err(&bad).contains("op not implemented: 'add-mesh'"));
    assert_eq!(b.pbo(), b.pristine);
    assert!(!b.root.join("state").exists());
}

#[test]
fn removes_and_copies_entries_and_tracks_sources() {
    let b = Boxed::new();
    fs::write(b.root.join("patches/replacement.bin"), b"new").unwrap();
    b.manifest(serde_json::json!([{ "path":"file.txt", "sha256":hash(b"a=1; 1;"), "remove":true }, { "path":"copy.txt", "copyOf":"file.txt", "sha256":hash(b"a=1; 1;"), "ops":[{"op":"replace-file","source":"replacement.bin"}] }]));
    let result = b.run("apply", &[]);
    assert_eq!(result.status.code(), Some(0), "{}", err(&result));
    let pbo = Pbo::parse(b.pbo()).unwrap();
    assert!(pbo.find(b"file.txt").unwrap().is_none());
    let copy = pbo.find(b"copy.txt").unwrap().unwrap();
    assert_eq!(pbo.data(copy).unwrap(), b"new");
    fs::write(b.root.join("patches/replacement.bin"), b"other").unwrap();
    assert_eq!(b.run("apply", &[]).status.code(), Some(0));
    let pbo = Pbo::parse(b.pbo()).unwrap();
    assert_eq!(
        pbo.data(pbo.find(b"copy.txt").unwrap().unwrap()).unwrap(),
        b"other"
    );
}
