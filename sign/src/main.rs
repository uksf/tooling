use std::{
    env,
    fs::File,
    path::{Path, PathBuf},
    process::ExitCode,
};

use hemtt_common::BISignVersion;
use hemtt_signing::{BIPrivateKey, BIPublicKey, BISign};
use pbo::dssign_hashes;
use signature::SigningKey;

mod pbo;
mod signature;

const USAGE: &str = "usage:
  uksf-sign create-key <name>                     writes <name>.biprivatekey and <name>.bikey to the current folder
  uksf-sign sign <key.biprivatekey> <file.pbo>... writes <file.pbo>.<name>.bisign next to each pbo
  uksf-sign verify <key.bikey> <file.pbo>...      checks <file.pbo>.<name>.bisign for each pbo";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("create-key") if args.len() == 2 => create_key(&args[1]),
        Some("sign") if args.len() >= 3 => sign(&args[1], &args[2..]),
        Some("verify") if args.len() >= 3 => verify(&args[1], &args[2..]),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn create_key(name: &str) -> Result<(), String> {
    let private =
        BIPrivateKey::generate(1024, name).map_err(|e| format!("generate {name}: {e}"))?;
    let private_path = format!("{name}.biprivatekey");
    private
        .write_danger(&mut create_secret(&private_path)?)
        .map_err(|e| format!("{private_path}: {e}"))?;
    let public_path = format!("{name}.bikey");
    private
        .to_public_key()
        .write(&mut create(&public_path)?)
        .map_err(|e| format!("{public_path}: {e}"))
}

fn sign(key_path: &str, pbos: &[String]) -> Result<(), String> {
    let bytes = std::fs::read(key_path).map_err(|e| format!("{key_path}: {e}"))?;
    let key = SigningKey::read(&bytes).map_err(|e| format!("{key_path}: {e}"))?;
    for pbo_path in pbos {
        let hashes = dssign_hashes(pbo_path, BISignVersion::V3)?;
        let output = signature_path(pbo_path, key.authority())?;
        key.write_signature(&mut create(&output.to_string_lossy())?, &hashes)
            .map_err(|e| format!("{}: {e}", output.display()))?;
    }
    Ok(())
}

fn verify(key_path: &str, pbos: &[String]) -> Result<(), String> {
    let key = BIPublicKey::read(&mut open(key_path)?).map_err(|e| format!("{key_path}: {e}"))?;
    for pbo_path in pbos {
        let path = signature_path(pbo_path, key.authority())?;
        let signature = BISign::read(&mut open(&path.to_string_lossy())?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if signature.length() != key.length()
            || signature.modulus() != key.modulus()
            || signature.exponent() != key.exponent()
        {
            return Err(format!(
                "{}: signed by a different key with authority {}",
                path.display(),
                key.authority()
            ));
        }
        if !signature::matches(&signature, &dssign_hashes(pbo_path, signature.version())?) {
            return Err(format!("{pbo_path}: signature does not match the PBO"));
        }
    }
    Ok(())
}

fn signature_path(pbo_path: &str, authority: &str) -> Result<PathBuf, String> {
    let name = Path::new(pbo_path)
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or(format!("{pbo_path}: not a file name"))?;
    Ok(Path::new(pbo_path).with_file_name(format!("{name}.{authority}.bisign")))
}

fn open(path: &str) -> Result<File, String> {
    File::open(path).map_err(|e| format!("{path}: {e}"))
}

fn create(path: &str) -> Result<File, String> {
    File::create(path).map_err(|e| format!("{path}: {e}"))
}

#[cfg(unix)]
fn create_secret(path: &str) -> Result<File, String> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("{path}: {e}"))?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(file)
}

#[cfg(not(unix))]
fn create_secret(path: &str) -> Result<File, String> {
    create(path)
}
