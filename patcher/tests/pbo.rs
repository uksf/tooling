use std::collections::{HashMap, HashSet};
use uksf_patcher::pbo::{Addition, Pbo};

fn fixture() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"\0sreV\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0prefix\0test/addon\0\0");
    b.extend_from_slice(b"one.txt\0");
    b.extend_from_slice(&[0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 42, 0, 0, 0, 3, 0, 0, 0]);
    b.extend_from_slice(b"\x80.bin\0");
    b.extend_from_slice(&[0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 2, 0, 0, 0]);
    b.extend_from_slice(&[0; 21]);
    b.extend_from_slice(b"abcxy");
    b.extend_from_slice(&[0; 21]);
    b
}

#[test]
fn rebuild_preserves_unedited_bytes_and_trailer() {
    let pbo = Pbo::parse(fixture()).unwrap();
    assert_eq!(pbo.find(b"TEST/ADDON/ONE.TXT").unwrap(), Some(0));
    assert_eq!(pbo.data(0).unwrap(), b"abc");
    let rebuilt = pbo.rebuild(&HashMap::new(), &[], &HashSet::new()).unwrap();
    let roundtrip = Pbo::parse(rebuilt).unwrap();
    assert!(roundtrip.trailer_valid());
    assert_eq!(
        &roundtrip.bytes[..roundtrip.bytes.len() - 21],
        &fixture()[..fixture().len() - 21]
    );
    assert_eq!(roundtrip.entries[1].name, b"\x80.bin");
}

#[test]
fn replacements_additions_and_removals_keep_expected_data() {
    let pbo = Pbo::parse(fixture()).unwrap();
    let replacements = HashMap::from([(0, b"hello".to_vec())]);
    let additions = [Addition {
        name: b"new".to_vec(),
        timestamp: 42,
        data: b"new data".to_vec(),
    }];
    let output = pbo
        .rebuild(&replacements, &additions, &HashSet::from([1]))
        .unwrap();
    let result = Pbo::parse(output).unwrap();
    assert!(result.trailer_valid());
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].timestamp, 42);
    assert_eq!(result.data(0).unwrap(), b"hello");
    assert_eq!(result.entries[1].name, b"new");
    assert_eq!(result.data(1).unwrap(), b"new data");
}

#[test]
fn rejects_truncated_pbo() {
    assert!(Pbo::parse(b"bad".to_vec()).is_err());
    let mut b = fixture();
    b.pop();
    assert!(Pbo::parse(b).is_err());
}
