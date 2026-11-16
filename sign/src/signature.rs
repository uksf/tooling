use std::io::Write;

use hemtt_common::BISignVersion;
use hemtt_signing::{BIPrivateKey, BIPublicKey, BISign, modpow, pad_hash, write_boxeduint};
use rsa::BoxedUint;

use crate::pbo::Hash;

pub struct SigningKey {
    public: BIPublicKey,
    private_exponent: BoxedUint,
}

impl SigningKey {
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        let key = BIPrivateKey::read(&mut &bytes[..]).map_err(|e| e.to_string())?;
        let half = (key.length() / 16) as usize;
        let start = key.authority().len() + 1 + 4 + 8 + 4 + 4 + 4 + 2 * half + 5 * half;
        let exponent = bytes
            .get(start..start + 2 * half)
            .ok_or("private key is shorter than its declared length")?;
        Ok(Self {
            public: key.to_public_key(),
            private_exponent: BoxedUint::from_le_slice_vartime(exponent),
        })
    }

    pub fn authority(&self) -> &str {
        self.public.authority()
    }

    pub fn write_signature<O: Write>(
        &self,
        output: &mut O,
        hashes: &[Hash; 3],
    ) -> std::io::Result<()> {
        let length = self.public.length();
        let size = (length / 8) as usize;
        let [sig1, sig2, sig3] = hashes.map(|hash| {
            modpow(
                &pad_hash(&hash, size),
                &self.private_exponent,
                self.public.modulus(),
            )
        });
        let block = (length / 8).to_le_bytes();
        output.write_all(self.authority().as_bytes())?;
        output.write_all(&[0])?;
        output.write_all(&(length / 8 + 20).to_le_bytes())?;
        output.write_all(b"\x06\x02\x00\x00\x00\x24\x00\x00RSA1")?;
        output.write_all(&length.to_le_bytes())?;
        write_boxeduint(output, self.public.exponent(), 4).map_err(std::io::Error::other)?;
        write_boxeduint(output, self.public.modulus(), size).map_err(std::io::Error::other)?;
        output.write_all(&block)?;
        write_boxeduint(output, &sig1, size).map_err(std::io::Error::other)?;
        output.write_all(&u32::from(BISignVersion::V3).to_le_bytes())?;
        output.write_all(&block)?;
        write_boxeduint(output, &sig2, size).map_err(std::io::Error::other)?;
        output.write_all(&block)?;
        write_boxeduint(output, &sig3, size).map_err(std::io::Error::other)
    }
}

pub fn matches(signature: &BISign, hashes: &[Hash; 3]) -> bool {
    let size = (signature.length() / 8) as usize;
    let (sig1, sig2, sig3) = signature.signatures_modpow();
    let [hash1, hash2, hash3] = hashes.map(|hash| pad_hash(&hash, size));
    sig1 == hash1 && sig2 == hash2 && sig3 == hash3
}
