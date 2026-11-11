pub fn pad3(n: i32) -> String {
    let sign = if n < 0 { "-" } else { "" };
    format!("{sign}{:03}", n.unsigned_abs())
}

fn special(x: f32) -> Option<String> {
    if x.is_nan() {
        Some("NaN".into())
    } else if x.is_infinite() {
        Some(if x > 0.0 { "Infinity" } else { "-Infinity" }.into())
    } else {
        None
    }
}

fn render(scientific: &str, max_digits: Option<usize>) -> String {
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((scientific, "0"));
    let (sign, mantissa) = mantissa
        .strip_prefix('-')
        .map_or(("", mantissa), |m| ("-", m));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    if digits.is_empty() {
        return format!("{sign}0");
    }
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let limit = max_digits.unwrap_or(digits.len().max(9)) as i32;
    let point = exponent + 1;
    if point > limit || point < -3 {
        let (head, tail) = digits.split_at(1);
        let dot = if tail.is_empty() { "" } else { "." };
        let mark = if exponent < 0 { '-' } else { '+' };
        format!(
            "{sign}{head}{dot}{tail}E{mark}{:02}",
            exponent.unsigned_abs()
        )
    } else if point <= 0 {
        format!(
            "{sign}0.{}{digits}",
            "0".repeat(point.unsigned_abs() as usize)
        )
    } else if point as usize >= digits.len() {
        format!(
            "{sign}{digits}{}",
            "0".repeat(point as usize - digits.len())
        )
    } else {
        let (whole, fraction) = digits.split_at(point as usize);
        format!("{sign}{whole}.{fraction}")
    }
}

pub fn float(x: f32) -> String {
    special(x).unwrap_or_else(|| render(&format!("{x:e}"), None))
}

pub fn general(x: f32, precision: usize) -> String {
    special(x).unwrap_or_else(|| {
        render(
            &format!("{:.*e}", precision - 1, f64::from(x)),
            Some(precision),
        )
    })
}

pub fn fixed(x: f32, decimals: usize) -> String {
    special(x).unwrap_or_else(|| format!("{x:.decimals$}"))
}

pub fn vector3(v: [f32; 3]) -> String {
    format!("<{}, {}, {}>", float(v[0]), float(v[1]), float(v[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad3_matches_dotnet_custom_000() {
        for (n, s) in [
            (-1, "-001"),
            (-99, "-099"),
            (-100, "-100"),
            (0, "000"),
            (1, "001"),
            (999, "999"),
            (1000, "1000"),
            (-1000, "-1000"),
            (i32::MIN, "-2147483648"),
            (i32::MAX, "2147483647"),
        ] {
            assert_eq!(pad3(n), s, "{n}");
        }
    }

    #[test]
    fn floats_match_dotnet_text() {
        for (bits, default, g6, g9, f4) in [
            (0x0000_0000, "0", "0", "0", "0.0000"),
            (0x8000_0000, "-0", "-0", "-0", "-0.0000"),
            (0x3dcc_cccd, "0.1", "0.1", "0.100000001", "0.1000"),
            (0xc020_0000, "-2.5", "-2.5", "-2.5", "-2.5000"),
            (0x42f6_e979, "123.456", "123.456", "123.456001", "123.4560"),
            (
                0x4b18_9680,
                "10000000",
                "1E+07",
                "10000000",
                "10000000.0000",
            ),
            (
                0x4996_b438,
                "1234567",
                "1.23457E+06",
                "1234567",
                "1234567.0000",
            ),
            (0x4b18_967f, "9999999", "1E+07", "9999999", "9999999.0000"),
            (
                0x4b3c_614e,
                "12345678",
                "1.23457E+07",
                "12345678",
                "12345678.0000",
            ),
            (0x38d1_b717, "0.0001", "0.0001", "9.99999975E-05", "0.0001"),
            (0x3727_c5ac, "1E-05", "1E-05", "9.99999975E-06", "0.0000"),
            (
                0x3901_742d,
                "0.00012345678",
                "0.000123457",
                "0.000123456775",
                "0.0001",
            ),
            (
                0x5863_5fa9,
                "1E+15",
                "1E+15",
                "9.99999987E+14",
                "999999986991104.0000",
            ),
            (
                0x7f7f_ffff,
                "3.4028235E+38",
                "3.40282E+38",
                "3.40282347E+38",
                "340282346638528859811704183484516925440.0000",
            ),
            (
                0x0000_0001,
                "1E-45",
                "1.4013E-45",
                "1.40129846E-45",
                "0.0000",
            ),
            (0xffc0_0000, "NaN", "NaN", "NaN", "NaN"),
            (0x7f80_0000, "Infinity", "Infinity", "Infinity", "Infinity"),
            (
                0xff80_0000,
                "-Infinity",
                "-Infinity",
                "-Infinity",
                "-Infinity",
            ),
            (0x4489_8000, "1100", "1100", "1100", "1100.0000"),
            (
                0x5511_84e7,
                "1E+13",
                "1E+13",
                "9.99999983E+12",
                "9999999827968.0000",
            ),
            (
                0x4974_23fb,
                "999999.7",
                "1E+06",
                "999999.688",
                "999999.6875",
            ),
            (
                0x4974_23f6,
                "999999.4",
                "999999",
                "999999.375",
                "999999.3750",
            ),
            (0x3f7f_ffff, "0.99999994", "1", "0.99999994", "1.0000"),
            (
                0x5a55_29af,
                "1.5E+16",
                "1.5E+16",
                "1.50000004E+16",
                "15000000408846336.0000",
            ),
            (0x4145_87e6, "12.345678", "12.3457", "12.3456783", "12.3457"),
            (
                0x00ef_8f0d,
                "2.2E-38",
                "2.2E-38",
                "2.20000005E-38",
                "0.0000",
            ),
            (0x3d00_0000, "0.03125", "0.03125", "0.03125", "0.0312"),
            (0x3dc0_0000, "0.09375", "0.09375", "0.09375", "0.0938"),
            (
                0x4996_b428,
                "1234565",
                "1.23456E+06",
                "1234565",
                "1234565.0000",
            ),
        ] {
            let x = f32::from_bits(bits);
            assert_eq!(
                [float(x), general(x, 6), general(x, 9), fixed(x, 4)],
                [default, g6, g9, f4],
                "{bits:#x}"
            );
        }
        assert_eq!(vector3([1.5, -0.0, 1e-5]), "<1.5, -0, 1E-05>");
    }
}
