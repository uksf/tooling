use uksf_patcher::odol::Header;

#[test]
fn reads_version_and_resolutions() {
    let mut data = b"ODOL".to_vec();
    data.extend_from_slice(&73u32.to_le_bytes());
    data.extend_from_slice(&42u32.to_le_bytes());
    data.extend_from_slice(b"flash\0");
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&0f32.to_le_bytes());
    data.extend_from_slice(&1.25f32.to_le_bytes());
    let header = Header::parse(&data).unwrap();
    assert_eq!(header.version, 73);
    assert_eq!(header.app_id, 42);
    assert_eq!(header.muzzle_flash, b"flash");
    assert_eq!(header.resolutions, [0.0, 1.25]);
    assert_eq!(header.model_info_offset, 30);
}

#[test]
fn rejects_encryption_and_truncated_headers() {
    let mut data = b"ODOL".to_vec();
    data.extend_from_slice(&75u32.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        Header::parse(&data).unwrap_err(),
        "ODOL v75 model is encrypted"
    );
    data.truncate(10);
    assert_eq!(Header::parse(&data).unwrap_err(), "truncated ODOL header");
}
