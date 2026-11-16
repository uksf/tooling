use std::{env, fs};

use sha1::{Digest, Sha1};

const CPRS: u32 = 0x4370_7273;
const VERS: u32 = 0x5665_7273;

fn header(out: &mut Vec<u8>, name: &[u8], fields: [u32; 5]) {
    out.extend_from_slice(name);
    out.push(0);
    for field in fields {
        out.extend_from_slice(&field.to_le_bytes());
    }
}

fn main() {
    let output = env::args().nth(1).expect("output path");
    let entries: [(&[u8], u32, Vec<u8>); 11] = [
        (b"zeta.sqf", 0, b"zeta script\n".to_vec()),
        (b"Alpha\\Upper.SQF", 0, b"upper case extension\n".to_vec()),
        (b"beta.sqf", CPRS, (7..47).collect()),
        (b"dir\\\xff\xfe.sqf", 0, b"not utf-8 name\n".to_vec()),
        (
            b"scenes\\intro.Lythium\\noext",
            0,
            b"no extension\n".to_vec(),
        ),
        (b".sqf", 0, b"dot file\n".to_vec()),
        (b"dup.paa", 0, b"first copy".to_vec()),
        (b"DUP.paa", 0, b"second copy".to_vec()),
        (b"empty.sqf", 0, Vec::new()),
        (b"data\\tex.paa", 0, (0..=255).collect()),
        (
            b"config.cpp",
            0,
            b"class CfgPatches { class uksf_dssign_fixture { units[] = {}; }; };\n".to_vec(),
        ),
    ];
    let mut out = Vec::new();
    header(&mut out, b"", [VERS, 0, 0, 0, 0]);
    out.extend_from_slice(b"prefix\0z\\uksf\\addons\\dssign_fixture\0\0");
    for (name, mime, data) in &entries {
        let original = if *mime == CPRS { 200 } else { 0 };
        let size = u32::try_from(data.len()).expect("small file");
        header(&mut out, name, [*mime, original, 0, 0x5F5E_1000, size]);
    }
    header(&mut out, b"", [0; 5]);
    for (_, _, data) in &entries {
        out.extend_from_slice(data);
    }
    let checksum = Sha1::digest(&out);
    out.push(0);
    out.extend_from_slice(&checksum);
    fs::write(output, out).expect("write fixture");
}
