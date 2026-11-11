use std::cmp::Ordering;

const PUNCTUATION: &str = "\t\n\u{b}\u{c}\r _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
const DIGIT: u32 = 100;
const LETTER: u32 = 200;
const OTHER: u32 = 1000;

fn latin1_letter(c: char) -> Option<(char, u8)> {
    const ACCENTED: [(char, char, &[u8]); 8] = [
        ('À', 'a', &[2, 1, 3, 6, 5, 4]),
        ('Ç', 'c', &[7]),
        ('È', 'e', &[2, 1, 3, 5]),
        ('Ì', 'i', &[2, 1, 3, 5]),
        ('Ñ', 'n', &[6]),
        ('Ò', 'o', &[2, 1, 3, 6, 5]),
        ('Ù', 'u', &[2, 1, 3, 5]),
        ('Ý', 'y', &[1]),
    ];
    if c == 'ÿ' {
        return Some(('y', 5));
    }
    let upper = if ('à'..='ý').contains(&c) {
        char::from_u32(c as u32 - 32)?
    } else {
        c
    };
    ACCENTED.iter().find_map(|&(first, base, accents)| {
        let at = (upper as u32).checked_sub(first as u32)? as usize;
        accents.get(at).map(|&a| (base, a))
    })
}

type Key = (Vec<u32>, Vec<u8>, Vec<u8>);

fn key(s: &str) -> Key {
    let mut key: Key = Default::default();
    for c in s.chars() {
        let upper = u8::from(c.is_uppercase());
        let (primary, secondary) = if let Some(i) = PUNCTUATION.find(c) {
            (1 + i as u32, 0)
        } else if c.is_ascii_digit() {
            (DIGIT + c as u32, 0)
        } else if c.is_ascii_alphabetic() {
            (LETTER + c.to_ascii_lowercase() as u32, 0)
        } else if let Some((base, accent)) = latin1_letter(c) {
            (LETTER + base as u32, accent)
        } else if c.is_control() {
            continue;
        } else {
            (OTHER + c as u32, 0)
        };
        key.0.push(primary);
        key.1.push(secondary);
        key.2.push(upper);
    }
    key
}

pub fn compare(a: &str, b: &str) -> Ordering {
    key(a).cmp(&key(b))
}

pub fn sorted(mut items: Vec<String>) -> Vec<String> {
    items.sort_by(|a, b| compare(a, b));
    items
}

#[cfg(test)]
mod tests {
    #[test]
    fn orders_like_dotnet_current_culture() {
        let csharp = [
            " ", "_x", "-z", "!", ".", "\"q\\", "~", "$", "1", "A", "À", "a b", "a_b", "a-b",
            "a,b", "a;b", "a:b", "a?b", "a.b", "a'b", "a(b", "a[b", "a{b", "a@b", "a*b", "a/b",
            "a\\b", "a&b", "a#b", "a%b", "a`b", "a^b", "a+b", "a<b", "a=b", "a>b", "a|b", "a~b",
            "a$b", "a0b", "a1", "a9b", "ab", "b", "B2", "co_op", "co-op", "coop", "Damage", "e",
            "é", "f", "xÿ", "Z",
        ];
        let mut shuffled: Vec<String> = csharp.iter().rev().map(|s| s.to_string()).collect();
        shuffled.rotate_left(17);
        assert_eq!(super::sorted(shuffled), csharp);
    }
}
