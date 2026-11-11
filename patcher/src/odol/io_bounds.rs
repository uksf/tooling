use super::Reader;

#[test]
fn int_array_rejects_huge_count() {
    let mut reader = Reader::new(&[0; 8]);
    assert_eq!(
        reader.i32s(0x3FFF_FFFF).unwrap_err(),
        "count 1073741823 of 4-byte elements exceeds the remaining 8 bytes at 0"
    );
}

#[test]
fn string_array_rejects_huge_count() {
    let mut data = 0x3FFF_FFFFu32.to_le_bytes().to_vec();
    data.extend_from_slice(&[0; 4]);
    let mut reader = Reader::new(&data);
    assert_eq!(
        reader.strings().unwrap_err(),
        "count 1073741823 of 1-byte elements exceeds the remaining 4 bytes at 4"
    );
}

#[test]
fn condensed_array_rejects_huge_count() {
    let mut data = 0x3FFF_FFFFu32.to_le_bytes().to_vec();
    data.push(1);
    data.extend_from_slice(&[0; 4]);
    let mut reader = Reader::new(&data);
    let err = match reader.condensed(4) {
        Err(err) => err,
        Ok(_) => panic!("expected condensed array to be rejected"),
    };
    assert_eq!(err, "declared size 4294967292 exceeds the 512 MiB cap at 5");
}
