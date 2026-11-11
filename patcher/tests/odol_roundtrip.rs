use uksf_patcher::odol::Model;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/data/{name}");
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn round_trip(name: &str) -> Model {
    let bytes = fixture(name);
    let model = Model::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    let written = model.write().unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(written, bytes, "{name} is not byte-identical");
    model
}

#[test]
fn committed_models_round_trip() {
    let scope = round_trip("scope.p3d");
    assert_eq!(scope.version, 75);
    assert_eq!(scope.lods.len(), 1);
    assert_eq!(scope.lods[0].resolution, 0.0);
    assert_eq!(scope.lods[0].vertices.len(), 20);
    assert_eq!(scope.lods[0].faces.len(), 6);

    let cable = round_trip("cable.p3d");
    assert_eq!(cable.lods.len(), 1);
    assert_eq!(cable.lods[0].resolution, 1.0);
    assert_eq!(cable.lods[0].faces.len(), 90);

    let casing = round_trip("casing.p3d");
    assert_eq!(casing.lods.len(), 4);
    assert_eq!(casing.lods[2].resolution, 2e15);

    let plug = round_trip("plug.p3d");
    assert_eq!(plug.lods.len(), 7);
    assert_eq!(plug.version, 75);
    let names: Vec<_> = plug
        .lods
        .iter()
        .flat_map(|lod| &lod.selections)
        .map(|selection| selection.name.as_slice())
        .collect();
    assert_eq!(names, [b"component01".as_slice(), b"rope_pos", b"tip"]);
}
