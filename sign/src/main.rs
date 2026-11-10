use std::{
    env,
    fs::File,
    path::{Path, PathBuf},
    process::ExitCode,
};

use hemtt_common::BISignVersion;
use hemtt_pbo::ReadablePbo;
use hemtt_signing::{BIPrivateKey, BIPublicKey, BISign};

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
        .write_danger(&mut create(&private_path)?)
        .map_err(|e| format!("{private_path}: {e}"))?;
    let public_path = format!("{name}.bikey");
    private
        .to_public_key()
        .write(&mut create(&public_path)?)
        .map_err(|e| format!("{public_path}: {e}"))
}

fn sign(key_path: &str, pbos: &[String]) -> Result<(), String> {
    let key = BIPrivateKey::read(&mut open(key_path)?).map_err(|e| format!("{key_path}: {e}"))?;
    for pbo_path in pbos {
        let mut pbo = read_pbo(pbo_path)?;
        let signature = key
            .sign(&mut pbo, BISignVersion::V3)
            .map_err(|e| format!("{pbo_path}: {e}"))?;
        let output = signature_path(pbo_path, key.authority())?;
        signature
            .write(&mut create(&output.to_string_lossy())?)
            .map_err(|e| format!("{}: {e}", output.display()))?;
    }
    Ok(())
}

fn verify(key_path: &str, pbos: &[String]) -> Result<(), String> {
    let key = BIPublicKey::read(&mut open(key_path)?).map_err(|e| format!("{key_path}: {e}"))?;
    for pbo_path in pbos {
        let mut pbo = read_pbo(pbo_path)?;
        let path = signature_path(pbo_path, key.authority())?;
        let signature = BISign::read(&mut open(&path.to_string_lossy())?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        key.verify(&mut pbo, &signature)
            .map_err(|e| format!("{pbo_path}: {e}"))?;
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

fn read_pbo(path: &str) -> Result<ReadablePbo<File>, String> {
    ReadablePbo::from(open(path)?).map_err(|e| format!("{path}: {e}"))
}

fn open(path: &str) -> Result<File, String> {
    File::open(path).map_err(|e| format!("{path}: {e}"))
}

fn create(path: &str) -> Result<File, String> {
    File::create(path).map_err(|e| format!("{path}: {e}"))
}
