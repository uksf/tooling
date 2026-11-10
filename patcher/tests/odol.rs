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

#[test]
fn reads_skeleton_bone_names_and_parents() {
    let mut data = b"ODOL".to_vec();
    data.extend_from_slice(&73u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.push(0);
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&[0; 216]);
    data.extend_from_slice(b"vehicle\0\x01\x02\0\0\0root\0\0turret\0root\0\0");
    let header = Header::parse(&data).unwrap();
    let skeleton = uksf_patcher::skeleton::Skeleton::parse(&data, &header).unwrap();
    assert_eq!(skeleton.name, b"vehicle");
    assert_eq!(
        skeleton.bones,
        [
            (b"root".to_vec(), vec![]),
            (b"turret".to_vec(), b"root".to_vec())
        ]
    );
    assert_eq!(skeleton.end, data.len());
}
