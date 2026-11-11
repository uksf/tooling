use crate::{
    engine,
    manifest::Manifest,
    pbo::Pbo,
    state::{State, Store, atomic_write, sha256},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Runner {
    pub addons: PathBuf,
    pub patches: PathBuf,
    pub store: Store,
}
impl Runner {
    pub fn apply(&self, dry: bool) -> Result<u8, String> {
        let _lock = self.store.lock()?;
        let manifests = Manifest::load_all(&self.patches)?;
        for manifest in &manifests {
            self.apply_one(manifest, dry)?;
        }
        for state in self.store.load_all()? {
            if !manifests
                .iter()
                .any(|m| m.pbo.eq_ignore_ascii_case(&state.pbo))
            {
                self.restore_orphan(&state, dry)?;
            }
        }
        Ok(0)
    }
    pub fn verify(&self) -> Result<u8, String> {
        let _lock = self.store.lock()?;
        let manifests = Manifest::load_all(&self.patches)?;
        let mut problems = 0;
        for manifest in &manifests {
            let current = self.read_pbo(&manifest.pbo)?;
            let sha = sha256(&current);
            let state = self.store.load(&manifest.pbo)?.map(|s| s.settled(&sha));
            let (pristine, _) = self.resolve(manifest, current, &sha, state.as_ref())?;
            if state.as_ref().is_none_or(|s| {
                s.patched_sha.as_deref() != Some(&sha)
                    || s.patch_set_sha.as_deref() != Some(&manifest.patch_set_hash)
            }) {
                println!("{}: NOT APPLIED (run apply)", manifest.pbo);
                problems += 1;
                continue;
            }
            let rebuilt = sha256(&engine::patch(pristine, manifest)?);
            if rebuilt != sha {
                println!(
                    "{}: MISMATCH: re-applying the patch gives {rebuilt}, the PBO is {sha}",
                    manifest.pbo
                );
                problems += 1;
            } else {
                println!("{}: ok ({sha})", manifest.pbo);
            }
        }
        for state in self.store.load_all()? {
            if !manifests
                .iter()
                .any(|m| m.pbo.eq_ignore_ascii_case(&state.pbo))
            {
                println!(
                    "{}: patched but no manifest (run apply to restore pristine)",
                    state.pbo
                );
                problems += 1;
            }
        }
        Ok(u8::from(problems != 0))
    }
    fn apply_one(&self, m: &Manifest, dry: bool) -> Result<(), String> {
        let current = self.read_pbo(&m.pbo)?;
        let sha = sha256(&current);
        let stored = self.store.load(&m.pbo)?;
        let state = stored.clone().map(|s| s.settled(&sha));
        if let Some(s) = &state
            && s.patched_sha.as_deref() == Some(&sha)
            && s.patch_set_sha.as_deref() == Some(&m.patch_set_hash)
        {
            if !dry && state != stored {
                self.store.save_state(s)?;
            }
            println!("{}: up to date", m.pbo);
            return Ok(());
        }
        let (pristine, reason) = self.resolve(m, current, &sha, state.as_ref())?;
        let patched = engine::patch(pristine.clone(), m)?;
        let patched_sha = sha256(&patched);
        if dry {
            println!("{}: would patch ({reason}) -> {patched_sha}", m.pbo);
            return Ok(());
        }
        let ours = state
            .as_ref()
            .filter(|s| s.patched_sha.as_deref() == Some(&sha));
        let pending = State {
            pbo: m.pbo.clone(),
            pristine_sha: sha256(&pristine),
            patch_set_sha: ours.and_then(|s| s.patch_set_sha.clone()),
            patched_sha: ours.and_then(|s| s.patched_sha.clone()),
            pending_patch_set_sha: Some(m.patch_set_hash.clone()),
            pending_patched_sha: Some(patched_sha.clone()),
        };
        self.store.save(&pending, &pristine)?;
        atomic_write(&self.addons.join(&m.pbo), &patched)?;
        self.store.save_state(&pending.settled(&patched_sha))?;
        println!("{}: patched ({reason}) -> {patched_sha}", m.pbo);
        Ok(())
    }
    fn resolve(
        &self,
        m: &Manifest,
        current: Vec<u8>,
        sha: &str,
        state: Option<&State>,
    ) -> Result<(Vec<u8>, &'static str), String> {
        let (pristine, reason, source) =
            if let Some(s) = state.filter(|s| sha != s.pristine_sha && s.produced(sha)) {
                (
                    self.backup(s)?,
                    "patch set changed",
                    format!("the backup {}", self.store.backup_path(&s.pbo).display()),
                )
            } else {
                (
                    current,
                    if state.is_none() {
                        "first patch"
                    } else if state.is_some_and(|s| sha == s.pristine_sha) {
                        "PBO is pristine"
                    } else {
                        "upstream PBO changed"
                    },
                    self.addons.join(&m.pbo).display().to_string(),
                )
            };
        let actual = sha256(&pristine);
        if actual != m.pristine_sha {
            return Err(format!(
                "{}: the pristine PBO ({source}) does not match its pin: expected sha256 {}, actual {actual}. Upstream changed the PBO; review and re-pin the patch.",
                m.pbo, m.pristine_sha
            ));
        }
        Ok((pristine, reason))
    }
    fn backup(&self, state: &State) -> Result<Vec<u8>, String> {
        let path = self.store.backup_path(&state.pbo);
        let backup = fs::read(&path)
            .ok()
            .filter(|b| sha256(b) == state.pristine_sha)
            .ok_or_else(|| {
                format!(
                    "{}: the PBO is patched but its pristine backup {} is missing or corrupt",
                    state.pbo,
                    path.display()
                )
            })?;
        let pbo =
            Pbo::parse(backup).map_err(|e| format!("{}: cannot parse: {e}", path.display()))?;
        if !pbo.trailer_valid() {
            return Err(format!(
                "{}: the SHA-1 trailer is invalid; the PBO is damaged",
                path.display()
            ));
        }
        Ok(pbo.bytes)
    }
    fn restore_orphan(&self, state: &State, dry: bool) -> Result<(), String> {
        let path = self.addons.join(&state.pbo);
        let sha = fs::read(&path).ok().map(|b| sha256(&b));
        if sha.as_deref() == Some(&state.pristine_sha) {
            println!(
                "{}: manifest removed and the PBO is pristine; forgetting its state",
                state.pbo
            );
            if !dry {
                self.store.forget(&state.pbo, true)?;
            }
        } else if sha.as_deref().is_none_or(|s| !state.produced(s)) {
            println!(
                "{}: manifest removed and the PBO is not ours any more; forgetting its state, keeping the pristine backup {}",
                state.pbo,
                self.store.backup_path(&state.pbo).display()
            );
            if !dry {
                self.store.forget(&state.pbo, false)?;
            }
        } else {
            let backup = self.backup(state)?;
            println!(
                "{}: manifest removed; {} pristine",
                state.pbo,
                if dry { "would restore" } else { "restored" }
            );
            if !dry {
                atomic_write(&path, &backup)?;
                self.store.forget(&state.pbo, true)?;
            }
        }
        Ok(())
    }
    fn read_pbo(&self, pbo: &str) -> Result<Vec<u8>, String> {
        fs::read(self.addons.join(pbo))
            .map_err(|_| format!("{pbo} is not in {}", self.addons.display()))
    }
}

pub fn absolute(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut normal = PathBuf::new();
    for part in absolute.components() {
        match part {
            std::path::Component::ParentDir => {
                normal.pop();
            }
            std::path::Component::CurDir => (),
            other => normal.push(other.as_os_str()),
        }
    }
    Ok(normal)
}
