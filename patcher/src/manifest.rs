use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub struct Asset {
    pub path: String,
    pub sha: String,
    pub ops: Vec<Value>,
    pub copy_of: Option<String>,
    pub remove: bool,
}

pub struct Manifest {
    pub file: PathBuf,
    pub pbo: String,
    pub pristine_sha: String,
    pub assets: Vec<Asset>,
    pub patch_set_hash: String,
}

fn required<'a>(v: &'a Value, key: &str, file: &Path) -> Result<&'a Value, String> {
    v.get(key)
        .ok_or_else(|| format!("{}: missing \"{key}\"", file.display()))
}

fn string(v: &Value, key: &str, file: &Path) -> Result<String, String> {
    required(v, key, file)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{}: \"{key}\" must be a string", file.display()))
}

fn without_comments(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = bytes.to_vec();
    let mut i = 0;
    let mut quoted = false;
    while i < bytes.len() {
        if quoted && bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            quoted = !quoted;
            i += 1;
            continue;
        }
        if !quoted && bytes.get(i..i + 2) == Some(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                out[i] = b' ';
                i += 1;
            }
        } else if !quoted && bytes.get(i..i + 2) == Some(b"/*") {
            out[i] = b' ';
            out[i + 1] = b' ';
            i += 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                if bytes[i] != b'\n' && bytes[i] != b'\r' {
                    out[i] = b' ';
                }
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return Err("unterminated JSON comment".into());
            }
            out[i] = b' ';
            out[i + 1] = b' ';
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(out)
}

impl Manifest {
    pub fn load_all(dir: &Path) -> Result<Vec<Self>, String> {
        if !dir.is_dir() {
            return Err(format!(
                "patches directory '{}' does not exist",
                dir.display()
            ));
        }
        let mut paths = fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|x| x.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.retain(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("json"))
        });
        paths.sort_by_key(|p| p.to_string_lossy().to_lowercase());
        let mut seen = HashSet::new();
        let mut manifests = Vec::new();
        for path in paths {
            let m = Self::load(&path)?;
            if !seen.insert(m.pbo.to_lowercase()) {
                return Err(format!("more than one manifest targets {}", m.pbo));
            }
            manifests.push(m);
        }
        Ok(manifests)
    }

    pub fn load(file: &Path) -> Result<Self, String> {
        let bytes = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let json = without_comments(&bytes)
            .map_err(|e| format!("{}: invalid JSON: {e}", file.display()))?;
        let root = crate::op_json::parse(&json)
            .map_err(|e| format!("{}: invalid JSON: {e}", file.display()))?;
        if root.get("format").and_then(Value::as_i64) != Some(1) {
            return Err(format!("{}: \"format\" must be 1", file.display()));
        }
        let pbo = string(&root, "pbo", file)?;
        let pristine_sha = string(&root, "pristineSha256", file)?.to_lowercase();
        if pristine_sha.len() != 64 || !pristine_sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!(
                "{}: \"pristineSha256\" must be the 64-digit sha256 of the pristine PBO (uksf-patcher hash <pbo>)",
                file.display()
            ));
        }
        let array = required(&root, "assets", file)?
            .as_array()
            .ok_or("assets must be an array")?;
        let mut assets = Vec::new();
        for a in array {
            let path = string(a, "path", file)?;
            let sha = string(a, "sha256", file)?.to_lowercase();
            let remove = a.get("remove").and_then(Value::as_bool) == Some(true);
            if remove && (a.get("ops").is_some() || a.get("copyOf").is_some()) {
                return Err(format!(
                    "{}: {path}: a removed asset takes no \"ops\" or \"copyOf\"",
                    file.display()
                ));
            }
            let ops = if remove {
                Vec::new()
            } else {
                let ops = required(a, "ops", file)?
                    .as_array()
                    .ok_or("ops must be an array")?
                    .clone();
                if ops.is_empty() {
                    return Err(format!("{}: {path} has no ops", file.display()));
                }
                ops
            };
            let copy_of = match a.get("copyOf") {
                None => None,
                Some(v) => Some(
                    v.as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| {
                            format!(
                                "{}: {path}: \"copyOf\" must be a non-empty string",
                                file.display()
                            )
                        })?
                        .to_owned(),
                ),
            };
            assets.push(Asset {
                path,
                sha,
                ops,
                copy_of,
                remove,
            });
        }
        if assets.is_empty() {
            return Err(format!("{}: no assets", file.display()));
        }
        let mut hash = Sha256::new();
        hash.update(1i32.to_le_bytes());
        hash.update(&bytes);
        let mut sources = HashSet::new();
        for op in assets.iter().flat_map(|a| &a.ops) {
            if let Some(source) = op.get("source").and_then(Value::as_str)
                && sources.insert(source.to_owned())
            {
                let path = file.parent().unwrap_or(Path::new("")).join(source);
                let data = fs::read(path).map_err(|_| {
                    format!(
                        "{}: referenced file '{source}' does not exist",
                        file.display()
                    )
                })?;
                hash.update(data);
            }
        }
        Ok(Self {
            file: file.to_owned(),
            pbo,
            pristine_sha,
            assets,
            patch_set_hash: format!("{:x}", hash.finalize()),
        })
    }
}
