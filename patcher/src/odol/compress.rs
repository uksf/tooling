fn literal(out: &mut Vec<u8>, data: &[u8], initial: bool) {
    let n = data.len();
    if n == 0 {
        return;
    }
    if initial && n <= 238 {
        out.push((17 + n) as u8);
    } else if n <= 3 {
        let i = out.len() - 2;
        out[i] |= n as u8;
    } else if n <= 18 {
        out.push((n - 3) as u8);
    } else {
        out.push(0);
        let mut t = n - 18;
        while t > 255 {
            out.push(0);
            t -= 255;
        }
        out.push(t as u8);
    }
    out.extend_from_slice(data);
}

fn word(data: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(data[i..i + 4].try_into().unwrap())
}

fn core(data: &[u8], start: usize, len: usize, carry: usize, out: &mut Vec<u8>) -> usize {
    let mut dict = vec![0usize; 1 << 14];
    let mut ip = start + 4usize.saturating_sub(carry);
    let mut ii = start;
    let end = start + len - 20;
    let mut ti = carry;
    let mut advance = true;
    loop {
        if advance {
            ip += 1 + ((ip - ii) >> 5);
        }
        if ip >= end {
            break;
        }
        let dv = word(data, ip);
        let d = (dv.wrapping_mul(0x1824429d) >> 18) as usize;
        let pos = start + dict[d];
        dict[d] = ip - start;
        if dv != word(data, pos) {
            advance = true;
            continue;
        }
        ii -= ti;
        ti = 0;
        literal(out, &data[ii..ip], false);
        let mut ml = 4;
        let mut v = word(data, ip + ml) ^ word(data, pos + ml);
        if v == 0 {
            loop {
                ml += 4;
                v = word(data, ip + ml) ^ word(data, pos + ml);
                if ip + ml >= end {
                    break;
                }
                if v != 0 {
                    ml += v.trailing_zeros() as usize / 8;
                    break;
                }
            }
        } else {
            ml += v.trailing_zeros() as usize / 8;
        }
        let mut off = ip - pos;
        ip += ml;
        ii = ip;
        if ml <= 8 && off <= 0x800 {
            off -= 1;
            out.push((((ml - 1) << 5) | ((off & 7) << 2)) as u8);
            out.push((off >> 3) as u8);
        } else if off <= 0x4000 {
            off -= 1;
            if ml <= 33 {
                out.push((32 | (ml - 2)) as u8);
            } else {
                ml -= 33;
                out.push(32);
                while ml > 255 {
                    ml -= 255;
                    out.push(0);
                }
                out.push(ml as u8);
            }
            out.push((off << 2) as u8);
            out.push((off >> 6) as u8);
        } else {
            off -= 0x4000;
            if ml <= 9 {
                out.push((16 | ((off >> 11) & 8) | (ml - 2)) as u8);
            } else {
                ml -= 9;
                out.push((16 | ((off >> 11) & 8)) as u8);
                while ml > 255 {
                    ml -= 255;
                    out.push(0);
                }
                out.push(ml as u8);
            }
            out.push((off << 2) as u8);
            out.push((off >> 6) as u8);
        }
        advance = false;
    }
    start + len - (ii - ti)
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut remaining = data.len();
    let mut carry = 0;
    while remaining > 20 {
        let len = remaining.min(49152);
        if (carry + len) >> 5 == 0 {
            break;
        }
        carry = core(data, start, len, carry, &mut out);
        start += len;
        remaining -= len;
    }
    carry += remaining;
    let initial = out.is_empty();
    literal(&mut out, &data[data.len() - carry..], initial);
    out.extend_from_slice(&[17, 0, 0]);
    out
}
