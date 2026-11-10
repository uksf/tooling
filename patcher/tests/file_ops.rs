use uksf_patcher::file_ops::text_replace;

#[test]
fn replaces_nonoverlapping_occurrences_and_preserves_latin1() {
    assert_eq!(
        text_replace(b"\xffabc abc", "abc", "caf\u{e9}", 2).unwrap(),
        b"\xffcaf\xe9 caf\xe9"
    );
    assert_eq!(text_replace(b"aaaa", "aa", "x", 2).unwrap(), b"xx");
}

#[test]
fn rejects_invalid_inputs() {
    assert!(text_replace(b"abc", "", "x", 1).is_err());
    assert!(text_replace(b"\0raPdata", "a", "x", 1).is_err());
    assert_eq!(
        text_replace(b"abc", "a", "x", 2).unwrap_err(),
        "text-replace expected 2 match(es) of \"a\", found 1"
    );
}
