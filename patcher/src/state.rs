use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct State {
    pub pbo: String,
    #[serde(rename = "PristineSha256")]
    pub pristine_sha: String,
    #[serde(rename = "PatchSetSha256")]
    pub patch_set_sha: Option<String>,
    #[serde(rename = "PatchedSha256")]
    pub patched_sha: Option<String>,
    #[serde(rename = "PendingPatchSetSha256")]
    pub pending_patch_set_sha: Option<String>,
    #[serde(rename = "PendingPatchedSha256")]
    pub pending_patched_sha: Option<String>,
}

impl State {
    pub fn produced(&self, sha: &str) -> bool {
        self.patched_sha.as_deref() == Some(sha) || self.pending_patched_sha.as_deref() == Some(sha)
    }
    pub fn settled(mut self, sha: &str) -> Self {
        if self.pending_patched_sha.as_deref() == Some(sha) {
            self.patch_set_sha = self.pending_patch_set_sha.take();
            self.patched_sha = self.pending_patched_sha.take();
        } else if self.pending_patched_sha.is_some() && self.patched_sha.as_deref() == Some(sha) {
            self.pending_patch_set_sha = None;
            self.pending_patched_sha = None;
        }
        self
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut temp = path.as_os_str().to_os_string();
    temp.push(".uksf-patcher.tmp");
    let temp = PathBuf::from(temp);
    let mut file = File::create(&temp).map_err(|e| format!("{}: {e}", temp.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("{}: {e}", temp.display()))?;
    drop(file);
    fs::rename(&temp, path).map_err(|e| format!("{}: {e}", path.display()))
}

pub struct Store {
    pub dir: PathBuf,
}
impl Store {
    pub fn state_path(&self, pbo: &str) -> PathBuf {
        self.dir.join(format!("{pbo}.state.json"))
    }
    pub fn backup_path(&self, pbo: &str) -> PathBuf {
        self.dir.join(format!("{pbo}.pristine"))
    }
    pub fn load(&self, pbo: &str) -> Result<Option<State>, String> {
        let path = self.state_path(pbo);
        if !path.exists() {
            return Ok(None);
        }
        self.read_state(&path).map(Some)
    }
    fn read_state(&self, path: &Path) -> Result<State, String> {
        let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&data).map_err(|e| format!("{}: {e}", path.display()))
    }
    pub fn load_all(&self) -> Result<Vec<State>, String> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut states = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".state.json"))
            {
                states.push(self.read_state(&path)?);
            }
        }
        Ok(states)
    }
    pub fn save_state(&self, state: &State) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
        atomic_write(&self.state_path(&state.pbo), &bytes)
    }
    pub fn save(&self, state: &State, pristine: &[u8]) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.backup_path(&state.pbo);
        if fs::read(&path)
            .map(|b| sha256(&b) != state.pristine_sha)
            .unwrap_or(true)
        {
            atomic_write(&path, pristine)?;
        }
        self.save_state(state)
    }
    pub fn forget(&self, pbo: &str, backup: bool) -> Result<(), String> {
        for path in [
            Some(self.state_path(pbo)),
            backup.then(|| self.backup_path(pbo)),
        ]
        .into_iter()
        .flatten()
        {
            match fs::remove_file(&path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Ok(())
    }
}
