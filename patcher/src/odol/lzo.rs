struct Dec<'a> {
    input: &'a [u8],
    ip: usize,
    out: Vec<u8>,
    op: usize,
}

enum Go {
    B3,
    First,
    Match(u32),
    MatchNext(u32),
    MatchDone,
    Done,
}

impl<'a> Dec<'a> {
    fn peek(&self) -> Result<u8, String> {
        self.input
            .get(self.ip)
            .copied()
            .ok_or_else(|| "truncated LZO".into())
    }

    fn peek_back(&self, n: usize) -> Result<u8, String> {
        self.ip
            .checked_sub(n)
            .and_then(|at| self.input.get(at))
            .copied()
            .ok_or_else(|| "truncated LZO".into())
    }

    fn next(&mut self) -> Result<u8, String> {
        let byte = self.peek()?;
        self.ip += 1;
        Ok(byte)
    }

    fn put(&mut self, byte: u8) -> Result<(), String> {
        let slot = self.out.get_mut(self.op).ok_or("LZO output overrun")?;
        *slot = byte;
        self.op += 1;
        Ok(())
    }

    fn copy_lit(&mut self, n: u32) -> Result<(), String> {
        for _ in 0..n {
            let byte = self.next()?;
            self.put(byte)?;
        }
        Ok(())
    }

    fn copy_from(&mut self, mut from: usize, n: u32) -> Result<(), String> {
        if n > 0 && from >= self.op {
            return Err("LZO lookbehind overrun".into());
        }
        for _ in 0..n {
            let byte = *self.out.get(from).ok_or("LZO lookbehind overrun")?;
            self.put(byte)?;
            from += 1;
        }
        Ok(())
    }

    fn extend(&mut self, mut value: u32, add: u32) -> Result<u32, String> {
        if value == 0 {
            while self.peek()? == 0 {
                value = value.checked_add(255).ok_or("LZO length overflow")?;
                self.ip += 1;
            }
            let extra = u32::from(self.next()?);
            value = value
                .checked_add(add)
                .and_then(|v| v.checked_add(extra))
                .ok_or("LZO length overflow")?;
        }
        Ok(value)
    }

    fn back(&self, dist: u32) -> Result<usize, String> {
        self.op
            .checked_sub(dist as usize)
            .ok_or_else(|| "LZO lookbehind overrun".into())
    }
}

pub fn decompress(input: &[u8], expected: usize) -> Result<(Vec<u8>, usize), String> {
    if expected == 0 {
        return Ok((Vec::new(), 0));
    }
    crate::bound::expand_fits(expected, input.len(), crate::bound::LZO_RATIO)?;
    let mut dec = Dec {
        input,
        ip: 0,
        out: vec![0; expected],
        op: 0,
    };
    let mut go = if dec.peek()? > 17 {
        let length = u32::from(dec.next()?) - 17;
        if length < 4 {
            Go::MatchNext(length)
        } else {
            dec.copy_lit(length)?;
            Go::First
        }
    } else {
        Go::B3
    };
    loop {
        go = match go {
            Go::Done => break,
            Go::B3 => {
                let instruction = u32::from(dec.next()?);
                if instruction >= 16 {
                    Go::Match(instruction)
                } else {
                    let length = dec.extend(instruction, 15)?;
                    dec.copy_lit(length + 3)?;
                    Go::First
                }
            }
            Go::First => {
                let instruction = u32::from(dec.next()?);
                if instruction >= 16 {
                    Go::Match(instruction)
                } else {
                    let offset = dec.next()?;
                    let dist = 1 + 0x800 + (instruction >> 2) + (u32::from(offset) << 2);
                    let from = dec.back(dist)?;
                    dec.copy_from(from, 3)?;
                    Go::MatchDone
                }
            }
            Go::Match(instruction) => match_at(&mut dec, instruction)?,
            Go::MatchNext(length) => {
                dec.copy_lit(length)?;
                Go::Match(u32::from(dec.next()?))
            }
            Go::MatchDone => {
                let extra = u32::from(dec.peek_back(2)?) & 3;
                if extra == 0 {
                    Go::B3
                } else {
                    Go::MatchNext(extra)
                }
            }
        };
    }
    if dec.op != expected {
        return Err(format!("LZO output underrun {}/{expected}", dec.op));
    }
    Ok((dec.out, dec.ip))
}

fn match_at(dec: &mut Dec<'_>, instruction: u32) -> Result<Go, String> {
    if instruction >= 64 {
        let offset = dec.next()?;
        let dist = 1 + ((instruction >> 2) & 7) + (u32::from(offset) << 3);
        let from = dec.back(dist)?;
        dec.copy_from(from, (instruction >> 5) - 1 + 2)?;
        return Ok(Go::MatchDone);
    }
    if instruction >= 32 {
        let length = dec.extend(instruction & 31, 31)?;
        let first = dec.next()?;
        let second = dec.next()?;
        let dist = 1 + (u32::from(first) >> 2) + (u32::from(second) << 6);
        let from = dec.back(dist)?;
        dec.copy_from(from, length + 2)?;
        return Ok(Go::MatchDone);
    }
    if instruction >= 16 {
        let mut dist = (instruction & 8) << 11;
        let length = dec.extend(instruction & 7, 7)?;
        let first = dec.next()?;
        let second = dec.next()?;
        dist += (u32::from(first) >> 2) + (u32::from(second) << 6);
        if dist == 0 {
            if dec.op != dec.out.len() {
                return Err("LZO output underrun".into());
            }
            return Ok(Go::Done);
        }
        let from = dec.back(dist + 0x4000)?;
        dec.copy_from(from, length + 2)?;
        return Ok(Go::MatchDone);
    }
    let offset = dec.next()?;
    let dist = 1 + (instruction >> 2) + (u32::from(offset) << 2);
    let from = dec.back(dist)?;
    dec.copy_from(from, 2)?;
    Ok(Go::MatchDone)
}

#[cfg(test)]
mod bounds {
    use super::decompress;

    #[test]
    fn lzo_block_rejects_huge_declared_size() {
        assert_eq!(
            decompress(&[0; 8], 0xFFFF_FFF0).unwrap_err(),
            "declared size 4294967280 exceeds the 512 MiB cap"
        );
    }
}
