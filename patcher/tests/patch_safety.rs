use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
struct Sandbox {
    root: PathBuf,
    pristine: Vec<u8>,
}
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "patcher-safety-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("mod/addons")).unwrap();
        fs::create_dir_all(root.join("patches")).unwrap();
        let mut pristine = b"a.txt\0".to_vec();
        for value in [0u32, 3, 0, 42, 3] {
            pristine.extend_from_slice(&value.to_le_bytes());
        }
        pristine.extend_from_slice(&[0; 21]);
        pristine.extend_from_slice(b"abc");
        let sha = Sha1::digest(&pristine);
        pristine.push(0);
        pristine.extend_from_slice(&sha);
        fs::write(root.join("mod/addons/x.pbo"), &pristine).unwrap();
        Self { root, pristine }
    }
    fn manifest(&self, replacement: &str) {
        let value = serde_json::json!({"format":1,"pbo":"x.pbo","pristineSha256":hash(&self.pristine),"assets":[{"path":"a.txt","sha256":hash(b"abc"),"ops":[{"op":"text-replace","find":"abc","replace":replacement}]}]});
        fs::write(
            self.root.join("patches/x.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    fn run(&self, command: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_uksf-patcher"))
            .arg(command)
            .arg("--addons")
            .arg(self.root.join("mod/addons"))
            .arg("--patches")
            .arg(self.root.join("patches"))
            .arg("--state")
            .arg(self.root.join("state"))
            .output()
            .unwrap()
    }
    fn state(&self) -> serde_json::Value {
        serde_json::from_slice(&fs::read(self.root.join("state/x.pbo.state.json")).unwrap())
            .unwrap()
    }
    fn save_state(&self, state: &serde_json::Value) {
        fs::write(
            self.root.join("state/x.pbo.state.json"),
            serde_json::to_vec(state).unwrap(),
        )
        .unwrap();
    }
    fn pbo(&self) -> Vec<u8> {
        fs::read(self.root.join("mod/addons/x.pbo")).unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn text(output: &[u8]) -> String {
    String::from_utf8_lossy(output).into_owned()
}

#[test]
fn interrupted_pending_install_commits_only_installed_output() {
    let b = Sandbox::new();
    b.manifest("first");
    assert!(b.run("apply").status.success());
    let a = b.pbo();
    let state_a = b.state();
    b.manifest("second");
    assert!(b.run("apply").status.success());
    let state_b = b.state();
    let mut pending = state_a.clone();
    pending["PendingPatchSetSha256"] = state_b["PatchSetSha256"].clone();
    pending["PendingPatchedSha256"] = state_b["PatchedSha256"].clone();
    b.save_state(&pending);
    let settled = b.run("apply");
    assert!(settled.status.success());
    assert!(text(&settled.stdout).contains("up to date"));
    assert_eq!(b.state(), state_b);
    fs::write(b.root.join("mod/addons/x.pbo"), a).unwrap();
    b.save_state(&pending);
    let rebuild = b.run("apply");
    assert!(rebuild.status.success(), "{}", text(&rebuild.stderr));
    assert!(text(&rebuild.stdout).contains("patch set changed"));
    assert_eq!(b.state(), state_b);
    assert!(b.run("verify").status.success());
}

#[test]
fn interrupted_first_install_retries_and_removed_manifest_restores() {
    let b = Sandbox::new();
    b.manifest("first");
    assert!(b.run("apply").status.success());
    let committed = b.state();
    let mut pending = committed.clone();
    pending["PendingPatchSetSha256"] = committed["PatchSetSha256"].clone();
    pending["PendingPatchedSha256"] = committed["PatchedSha256"].clone();
    pending["PatchSetSha256"] = serde_json::Value::Null;
    pending["PatchedSha256"] = serde_json::Value::Null;
    fs::write(b.root.join("mod/addons/x.pbo"), &b.pristine).unwrap();
    b.save_state(&pending);
    assert!(b.run("apply").status.success());
    assert_eq!(b.state(), committed);
    b.save_state(&pending);
    fs::remove_file(b.root.join("patches/x.json")).unwrap();
    assert!(b.run("apply").status.success());
    assert_eq!(b.pbo(), b.pristine);
    assert_eq!(fs::read_dir(b.root.join("state")).unwrap().count(), 0);
}

#[test]
fn foreign_pbo_keeps_backup_and_forgets_state() {
    let b = Sandbox::new();
    b.manifest("first");
    assert!(b.run("apply").status.success());
    fs::write(b.root.join("mod/addons/x.pbo"), b"foreign").unwrap();
    fs::remove_file(b.root.join("patches/x.json")).unwrap();
    let result = b.run("apply");
    assert!(result.status.success());
    assert!(text(&result.stdout).contains("keeping the pristine backup"));
    assert_eq!(b.pbo(), b"foreign");
    assert_eq!(
        fs::read(b.root.join("state/x.pbo.pristine")).unwrap(),
        b.pristine
    );
    assert!(!b.root.join("state/x.pbo.state.json").exists());
}

#[test]
fn bad_trailer_and_hardlinks_are_safe() {
    let b = Sandbox::new();
    let linked = b.root.join("linked.pbo");
    fs::hard_link(b.root.join("mod/addons/x.pbo"), &linked).unwrap();
    b.manifest("first");
    assert!(b.run("apply").status.success());
    assert_eq!(fs::read(&linked).unwrap(), b.pristine);
    assert_ne!(b.pbo(), b.pristine);
    let mut corrupt = b.pristine.clone();
    corrupt[47] ^= 1;
    fs::write(b.root.join("mod/addons/x.pbo"), &corrupt).unwrap();
    let path = b.root.join("patches/x.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["pristineSha256"] = hash(&corrupt).into();
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let result = b.run("apply");
    assert_eq!(result.status.code(), Some(1));
    assert!(text(&result.stderr).contains("SHA-1 trailer"));
    assert_eq!(b.pbo(), corrupt);
}
