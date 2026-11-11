fn trim(s: &str) -> &str {
    let white = |c: char| c == ' ' || ('\t'..='\r').contains(&c);
    s.trim_end_matches('\0')
        .trim_end_matches(white)
        .trim_start_matches(white)
}

pub fn parse_int(s: &str) -> Option<i32> {
    trim(s).parse().ok()
}

pub fn parse_float(s: &str) -> Option<f32> {
    let t = trim(s);
    let body = t.trim_start_matches(['+', '-']);
    let sign = &t[..t.len() - body.len()];
    let split = body.find(['.', 'e', 'E']).unwrap_or(body.len());
    let (whole, rest) = body.split_at(split);
    let valid = sign.len() <= 1
        && !whole.starts_with(',')
        && t.bytes()
            .all(|b| b.is_ascii_digit() || b",+-.eE".contains(&b));
    let digits = whole.replace(',', "");
    valid.then(|| format!("{sign}{digits}{rest}").parse().ok())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_matches_dotnet_invariant_float_and_int() {
        for (text, value) in [
            (" 1201 ", Some(1201.0)),
            ("1,201", Some(1201.0)),
            ("1,", Some(1.0)),
            (",5", None),
            ("1.e5", Some(100_000.0)),
            ("\t5\n", Some(5.0)),
            ("+5", Some(5.0)),
            ("- 5", None),
            ("1,2,3", Some(123.0)),
            ("1.5,0", None),
            ("0x10", None),
            ("1_0", None),
            ("1e", None),
            ("1,.5", Some(1.5)),
            ("\u{a0}5", None),
            ("+-5", None),
        ] {
            assert_eq!(parse_float(text), value, "{text:?}");
        }
        for (text, value) in [
            (" 5", Some(5)),
            ("5\0", Some(5)),
            ("-0", Some(0)),
            ("5 \0", Some(5)),
            ("5\0 ", None),
            ("05", Some(5)),
            ("\u{a0}5", None),
            ("5,0", None),
            (" -5 ", Some(-5)),
        ] {
            assert_eq!(parse_int(text), value, "{text:?}");
        }
    }
}
