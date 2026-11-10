use sha2::{Digest, Sha256};
use std::{env, fs, process::ExitCode};
use uksf_patcher::pbo::Pbo;

const USAGE: &str = "uksf-patcher apply  --addons <dir> --patches <dir> --state <dir> [--dry-run]\nuksf-patcher verify --addons <dir> --patches <dir> --state <dir>\nuksf-patcher inspect <pbo> [filter]\nuksf-patcher hash <pbo> [path-in-pbo]\nuksf-patcher p3d <pbo> <path-in-pbo> [lod]";

fn run(args: &[String]) -> Result<u8, String> {
    match args {
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
        _ => {
            eprintln!("{USAGE}");
            Ok(2)
        }
    }
}

fn read_pbo(path: &str) -> Result<Pbo, String> {
    Pbo::parse(fs::read(path).map_err(|e| format!("{path}: {e}"))?)
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
