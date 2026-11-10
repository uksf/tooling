use std::{env, fs::File, io::Cursor};

use hemtt_pbo::WritablePbo;

fn main() {
    let output = env::args().nth(1).expect("output path");
    let files: [(&str, Vec<u8>); 6] = [
        ("config.cpp", b"class CfgPatches { class uksf_sign_fixture { units[] = {}; }; };\n".to_vec()),
        ("functions/fnc_hello.sqf", b"#include \"..\\script_component.hpp\"\nparams [\"_name\"];\nformat [\"hello %1\", _name]\n".to_vec()),
        ("script_component.hpp", b"#define COMPONENT fixture\n".to_vec()),
        ("data/texture_co.paa", (0..=255u8).cycle().take(4096).collect()),
        ("data/Empty.SQF", Vec::new()),
        ("Readme.TXT", b"Fixture for uksf-sign golden tests.\n".to_vec()),
    ];
    let mut pbo = WritablePbo::<Cursor<Vec<u8>>>::new();
    pbo.add_property("prefix", "z\\uksf\\addons\\sign_fixture");
    for (name, data) in files {
        pbo.add_file(name, Cursor::new(data)).expect("add file");
    }
    let mut out = File::create(output).expect("create");
    pbo.write(&mut out, true).expect("write");
}
