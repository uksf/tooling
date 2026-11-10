use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use std::{env, fs, process::ExitCode};
use uksf_patcher::{
    pbo::Pbo,
    runner::{Runner, absolute},
    state::Store,
};

const USAGE: &str = "uksf-patcher apply  --addons <dir> --patches <dir> --state <dir> [--dry-run]\nuksf-patcher verify --addons <dir> --patches <dir> --state <dir>\nuksf-patcher inspect <pbo> [filter]\nuksf-patcher hash <pbo> [path-in-pbo]\nuksf-patcher p3d <pbo> <path-in-pbo> [lod]";

fn run(args: &[String]) -> Result<u8, String> {
    match args {
        [command, rest @ ..] if command == "apply" || command == "verify" => patcher(command, rest),
        [command, path] if command == "hash" => {
            let bytes = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            println!("{:x}", Sha256::digest(bytes));
            Ok(0)
        }
        [command, path, asset] if command == "hash" => {
            let pbo = read_pbo(path)?;
            let name = asset
                .chars()
                .map(|c| u8::try_from(c as u32))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| format!("{asset} is not in {path}"))?;
            let index = pbo
                .find(&name)?
                .ok_or_else(|| format!("{asset} is not in {path}"))?;
            println!("{:x}", Sha256::digest(pbo.data(index)?));
            Ok(0)
        }
        [command, path, rest @ ..] if command == "inspect" => {
            let pbo = read_pbo(path)?;
            println!(
                "header {} bytes, {} entries, trailer hash valid: {}",
                pbo.header_len,
                pbo.entries.len(),
                if pbo.trailer_valid() { "True" } else { "False" }
            );
            for (index, entry) in pbo.entries.iter().enumerate() {
                let name: String = entry.name.iter().map(|b| char::from(*b)).collect();
                if rest
                    .first()
                    .is_some_and(|filter| !name.to_lowercase().contains(&filter.to_lowercase()))
                {
                    continue;
                }
                println!(
                    "{index:6} {:08X} {:10} {:10} {name}",
                    entry.packing, entry.original_size, entry.size
                );
            }
            Ok(0)
        }
        [command, mode, path] if command == "p3d" && mode == "roundtrip" => roundtrip(path),
        [command, path, asset] if command == "p3d" => {
            uksf_patcher::p3d_info::run(path, asset, None)
        }
        [command, path, asset, lod] if command == "p3d" => {
            uksf_patcher::p3d_info::run(path, asset, Some(lod))
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(2)
        }
    }
}

fn patcher(command: &str, args: &[String]) -> Result<u8, String> {
    let mut options = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i]
            .strip_prefix("--")
            .ok_or_else(|| format!("unexpected argument '{}'", args[i]))?;
        if key == "dry-run" {
            options.insert(key, None);
            i += 1;
            continue;
        }
        i += 1;
        let value = args
            .get(i)
            .ok_or_else(|| format!("--{key} needs a value"))?;
        options.insert(key, Some(value));
        i += 1;
    }
    let get = |key| {
        options
            .get(key)
            .and_then(|v| *v)
            .map(PathBuf::from)
            .ok_or("--addons, --patches and --state are required".to_owned())
    };
    let addons = get("addons")?;
    let patches = get("patches")?;
    let state = get("state")?;
    let mod_dir = absolute(&addons.join(".."))?;
    let state_abs = absolute(Path::new(&state))?;
    let mod_fold = mod_dir.to_string_lossy().to_lowercase().replace('/', "\\");
    let state_fold = state_abs
        .to_string_lossy()
        .to_lowercase()
        .replace('/', "\\");
    if state_fold == mod_fold || state_fold.starts_with(&(mod_fold + "\\")) {
        return Err(
            "--state must be outside the mod folder, or the pristine backups ship with the mod"
                .into(),
        );
    }
    let runner = Runner {
        addons,
        patches,
        store: Store { dir: state },
    };
    if command == "verify" {
        runner.verify()
    } else {
        runner.apply(options.contains_key("dry-run"))
    }
}

fn read_pbo(path: &str) -> Result<Pbo, String> {
    Pbo::parse(fs::read(path).map_err(|e| format!("{path}: {e}"))?)
}

fn roundtrip(path: &str) -> Result<u8, String> {
    let bytes = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".p3d") || bytes.starts_with(b"ODOL") || bytes.starts_with(b"MLOD") {
        let identical = check_p3d(path, &bytes) == Outcome::Identical;
        println!("{path}: 1 p3ds, {} identical", u8::from(identical));
        return Ok(u8::from(!identical));
    }
    let pbo = Pbo::parse(bytes).map_err(|e| format!("{path}: {e}"))?;
    let mut total = 0;
    let mut identical = 0;
    let mut not_odol = 0;
    let mut unreadable = 0;
    for (index, entry) in pbo.entries.iter().enumerate() {
        if entry.name.len() < 4 || !entry.name[entry.name.len() - 4..].eq_ignore_ascii_case(b".p3d")
        {
            continue;
        }
        total += 1;
        let name = String::from_utf8_lossy(&entry.name);
        let label = format!("{path}:{name}");
        match pbo.data(index) {
            Ok(data) => match check_p3d(&label, &data) {
                Outcome::Identical => identical += 1,
                Outcome::NotOdol => not_odol += 1,
                Outcome::Mismatch(_) | Outcome::Failed => {}
            },
            Err(error) => {
                unreadable += 1;
                println!("{label}: {error}");
            }
        }
    }
    println!(
        "{path}: {total} p3ds, {identical} identical, {not_odol} not binarised, {unreadable} unreadable"
    );
    Ok(u8::from(identical + not_odol != total))
}

#[derive(PartialEq)]
enum Outcome {
    Identical,
    Mismatch(usize),
    NotOdol,
    Failed,
}

fn check_p3d(label: &str, bytes: &[u8]) -> Outcome {
    match uksf_patcher::odol::Model::parse(bytes).and_then(|model| model.write()) {
        Ok(written) => match uksf_patcher::odol::first_mismatch(bytes, &written) {
            None => Outcome::Identical,
            Some(offset) => {
                println!("{label}: mismatch at {offset}");
                Outcome::Mismatch(offset)
            }
        },
        Err(error) if error == "not a binarised (ODOL) P3D" => {
            println!("{label}: {error}");
            Outcome::NotOdol
        }
        Err(error) => {
            println!("{label}: {error}");
            Outcome::Failed
        }
    }
}

fn main() -> ExitCode {
    match run(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
