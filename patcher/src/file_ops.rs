pub fn text_replace(
    input: &[u8],
    find: &str,
    replacement: &str,
    expected: usize,
) -> Result<Vec<u8>, String> {
    if find.is_empty() {
        return Err("text-replace \"find\" is empty".into());
    }
    if input.starts_with(b"\0raP") {
        return Err(
            "text-replace cannot edit a rapified (binarised) config; use replace-file".into(),
        );
    }
    let encode = |value: &str| {
        value
            .chars()
            .map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
            .collect::<Vec<_>>()
    };
    let find = match find
        .chars()
        .map(|c| u8::try_from(c as u32))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(find) => find,
        Err(_) if expected == 0 => return Ok(input.to_vec()),
        Err(_) => {
            return Err(format!(
                "text-replace expected {expected} match(es) of \"{find}\", found 0"
            ));
        }
    };
    let find_text: String = find.iter().map(|byte| char::from(*byte)).collect();
    let replacement = encode(replacement);
    let mut output = Vec::new();
    let mut offset = 0;
    let mut count = 0;
    while let Some(index) = input[offset..]
        .windows(find.len())
        .position(|window| window == find)
    {
        output.extend_from_slice(&input[offset..offset + index]);
        output.extend_from_slice(&replacement);
        offset += index + find.len();
        count += 1;
    }
    if count != expected {
        return Err(format!(
            "text-replace expected {expected} match(es) of \"{find_text}\", found {count}"
        ));
    }
    output.extend_from_slice(&input[offset..]);
    Ok(output)
}
