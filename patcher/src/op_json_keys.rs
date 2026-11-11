use super::RESERVED_KEY;

pub fn has_reserved_key(json: &[u8]) -> bool {
    let mut i = 0;
    while i < json.len() {
        if json[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while json[i] != b'"' {
            i += if json[i] == b'\\' { 2 } else { 1 };
        }
        i += 1;
        let is_key = json[i..].iter().find(|b| !b.is_ascii_whitespace()) == Some(&b':');
        if is_key
            && serde_json::from_slice::<String>(&json[start..i]).is_ok_and(|k| k == RESERVED_KEY)
        {
            return true;
        }
    }
    false
}
