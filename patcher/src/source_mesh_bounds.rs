use super::read;

#[test]
fn mlod_rejects_huge_point_count() {
    let mut data = Vec::new();
    data.extend_from_slice(b"MLOD");
    data.extend_from_slice(&257u32.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(b"P3DM");
    data.extend_from_slice(&28u32.to_le_bytes());
    data.extend_from_slice(&256u32.to_le_bytes());
    data.extend_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&[0; 8]);
    assert_eq!(
        read(&data).unwrap_err(),
        "count 4294967280 of 16-byte elements exceeds the remaining 8 bytes"
    );
}
