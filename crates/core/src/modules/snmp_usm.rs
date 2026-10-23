//! SNMPv3's User-based Security Model (RFC 3414): keys from passwords,
//! message authentication and privacy.
//!
//! - Authentication: HMAC-MD5-96 and HMAC-SHA-96 (RFC 3414 §6, §7), and the
//!   HMAC-SHA-2 protocols of RFC 7860 (SHA-224, -256, -384, -512, truncated
//!   to 16, 24, 32 and 48 bytes).
//! - Keys: the password stretched over 1 MiB and hashed, then localized to
//!   an engine as H(Ku || engineID || Ku) (RFC 3414 §2.6, A.2), with the
//!   authentication protocol's hash for both keys (RFC 7860 §5).
//! - Privacy: CBC-DES (RFC 3414 §8), CFB128-AES-128 (RFC 3826), and AES-192
//!   and AES-256 as draft-blumenthal-aes-usm-04 defines them, with its
//!   extension of a localized key too short for the cipher:
//!   Kul = Kul || H(Kul), repeated.
//!
//! HMAC (RFC 2104) is computed here over each protocol's hash, so one
//! implementation serves all six.

use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use md5::Digest as _;
use sha1::Digest as _;

/// An authentication protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Auth {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

impl Auth {
    pub(crate) fn from_name(name: &str) -> Option<Auth> {
        Some(match name {
            "md5" => Auth::Md5,
            "sha" => Auth::Sha1,
            "sha224" => Auth::Sha224,
            "sha256" => Auth::Sha256,
            "sha384" => Auth::Sha384,
            "sha512" => Auth::Sha512,
            _ => return None,
        })
    }

    fn hash(self, parts: &[&[u8]]) -> Vec<u8> {
        macro_rules! digest {
            ($h:ty) => {{
                let mut h = <$h>::new();
                for p in parts {
                    h.update(p);
                }
                h.finalize().to_vec()
            }};
        }
        match self {
            Auth::Md5 => digest!(md5::Md5),
            Auth::Sha1 => digest!(sha1::Sha1),
            Auth::Sha224 => digest!(sha2::Sha224),
            Auth::Sha256 => digest!(sha2::Sha256),
            Auth::Sha384 => digest!(sha2::Sha384),
            Auth::Sha512 => digest!(sha2::Sha512),
        }
    }

    /// The hash's input block, for HMAC.
    fn block(self) -> usize {
        match self {
            Auth::Sha384 | Auth::Sha512 => 128,
            _ => 64,
        }
    }

    /// The length of msgAuthenticationParameters: the truncated HMAC.
    pub(crate) fn mac_len(self) -> usize {
        match self {
            Auth::Md5 | Auth::Sha1 => 12,
            Auth::Sha224 => 16,
            Auth::Sha256 => 24,
            Auth::Sha384 => 32,
            Auth::Sha512 => 48,
        }
    }

    /// Ku: the password stretched to 1 MiB by repetition and hashed
    /// (RFC 3414 A.2).
    pub(crate) fn password_key(self, password: &[u8]) -> Vec<u8> {
        assert!(!password.is_empty(), "a USM password is not empty");
        let mut stretched = Vec::with_capacity(1 << 20);
        while stretched.len() < 1 << 20 {
            let take = password.len().min((1 << 20) - stretched.len());
            stretched.extend_from_slice(&password[..take]);
        }
        self.hash(&[&stretched])
    }

    /// Kul: Ku localized to an engine (RFC 3414 §2.6).
    pub(crate) fn localize(self, ku: &[u8], engine_id: &[u8]) -> Vec<u8> {
        self.hash(&[ku, engine_id, ku])
    }

    /// HMAC (RFC 2104) with this protocol's hash, truncated to `mac_len`.
    pub(crate) fn mac(self, key: &[u8], message: &[u8]) -> Vec<u8> {
        let block = self.block();
        let mut k = if key.len() > block {
            self.hash(&[key])
        } else {
            key.to_vec()
        };
        k.resize(block, 0);
        let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
        let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
        let inner = self.hash(&[&ipad, message]);
        let mut out = self.hash(&[&opad, &inner]);
        out.truncate(self.mac_len());
        out
    }
}

/// A privacy protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Privacy {
    Des,
    Aes128,
    Aes192,
    Aes256,
}

impl Privacy {
    pub(crate) fn from_name(name: &str) -> Option<Privacy> {
        Some(match name {
            "des" => Privacy::Des,
            "aes" => Privacy::Aes128,
            "aes192" => Privacy::Aes192,
            "aes256" => Privacy::Aes256,
            _ => return None,
        })
    }

    /// Key material the cipher needs from the localized key.
    fn key_material(self) -> usize {
        match self {
            Privacy::Des | Privacy::Aes128 => 16,
            Privacy::Aes192 => 24,
            Privacy::Aes256 => 32,
        }
    }

    /// The localized key, extended as draft-blumenthal-aes-usm-04 §3.1.2.1
    /// does when the hash is shorter than the cipher needs, and cut to it.
    pub(crate) fn key(self, auth: Auth, localized: &[u8]) -> Vec<u8> {
        let mut key = localized.to_vec();
        while key.len() < self.key_material() {
            let more = auth.hash(&[&key]);
            key.extend(more);
        }
        key.truncate(self.key_material());
        key
    }

    /// Encrypt a scoped PDU with `key` (from [`Privacy::key`]), the
    /// authoritative engine's boots and time, and a salt never used before
    /// with this key. Returns the ciphertext and msgPrivacyParameters.
    pub(crate) fn encrypt(
        self,
        key: &[u8],
        boots: u32,
        time: u32,
        salt: u64,
        plain: &[u8],
    ) -> (Vec<u8>, Vec<u8>) {
        match self {
            Privacy::Des => {
                // RFC 3414 §8.1.1.1: the salt is engineBoots then a local
                // 32-bit integer; IV = pre-IV XOR salt.
                let mut params = boots.to_be_bytes().to_vec();
                params.extend_from_slice(&(salt as u32).to_be_bytes());
                let iv: Vec<u8> = key[8..16].iter().zip(&params).map(|(a, b)| a ^ b).collect();
                let mut data = plain.to_vec();
                data.resize(plain.len().div_ceil(8) * 8, 0);
                let cipher = des::Des::new_from_slice(&key[..8]).expect("an 8-byte DES key");
                let mut prev: [u8; 8] = iv.try_into().expect("8 bytes");
                for chunk in data.chunks_mut(8) {
                    for (b, p) in chunk.iter_mut().zip(prev) {
                        *b ^= p;
                    }
                    let block: &mut des::cipher::Block<des::Des> =
                        <&mut [u8; 8]>::try_from(&mut *chunk)
                            .expect("8 bytes")
                            .into();
                    cipher.encrypt_block(block);
                    prev.copy_from_slice(chunk);
                }
                (data, params)
            }
            _ => {
                let params = salt.to_be_bytes().to_vec();
                let iv = aes_iv(boots, time, &params);
                (aes_cfb(key, &iv, plain, true), params)
            }
        }
    }

    pub(crate) fn decrypt(
        self,
        key: &[u8],
        boots: u32,
        time: u32,
        params: &[u8],
        cipher_text: &[u8],
    ) -> Result<Vec<u8>, String> {
        if params.len() != 8 {
            return Err("msgPrivacyParameters is not 8 bytes".into());
        }
        match self {
            Privacy::Des => {
                if !cipher_text.len().is_multiple_of(8) {
                    return Err("a DES-encrypted PDU is not a whole number of blocks".into());
                }
                let iv: Vec<u8> = key[8..16].iter().zip(params).map(|(a, b)| a ^ b).collect();
                let cipher = des::Des::new_from_slice(&key[..8]).expect("an 8-byte DES key");
                let mut prev: [u8; 8] = iv.try_into().expect("8 bytes");
                let mut out = cipher_text.to_vec();
                for chunk in out.chunks_mut(8) {
                    let this: [u8; 8] = (&*chunk).try_into().expect("8 bytes");
                    let block: &mut des::cipher::Block<des::Des> =
                        <&mut [u8; 8]>::try_from(&mut *chunk)
                            .expect("8 bytes")
                            .into();
                    cipher.decrypt_block(block);
                    for (b, p) in chunk.iter_mut().zip(prev) {
                        *b ^= p;
                    }
                    prev = this;
                }
                Ok(out)
            }
            _ => {
                let iv = aes_iv(boots, time, params);
                Ok(aes_cfb(key, &iv, cipher_text, false))
            }
        }
    }
}

/// RFC 3826 §3.1.2.1: engineBoots, engineTime, then the 64-bit salt.
fn aes_iv(boots: u32, time: u32, salt: &[u8]) -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv[..4].copy_from_slice(&boots.to_be_bytes());
    iv[4..8].copy_from_slice(&time.to_be_bytes());
    iv[8..].copy_from_slice(salt);
    iv
}

/// CFB with 128-bit segments (NIST SP 800-38A §6.3), any length, no
/// padding: the last segment uses as many output bytes as it has.
fn aes_cfb(key: &[u8], iv: &[u8; 16], data: &[u8], encrypt: bool) -> Vec<u8> {
    let mut feedback = *iv;
    let mut out = Vec::with_capacity(data.len());
    let encrypt_block = |block: &mut aes::Block| match key.len() {
        16 => aes::Aes128::new_from_slice(key)
            .expect("16 bytes")
            .encrypt_block(block),
        24 => aes::Aes192::new_from_slice(key)
            .expect("24 bytes")
            .encrypt_block(block),
        _ => aes::Aes256::new_from_slice(key)
            .expect("32 bytes")
            .encrypt_block(block),
    };
    for chunk in data.chunks(16) {
        let mut block = aes::Block::from(feedback);
        encrypt_block(&mut block);
        let produced: Vec<u8> = chunk.iter().zip(block.iter()).map(|(d, k)| d ^ k).collect();
        let cipher_bytes = if encrypt { &produced[..] } else { chunk };
        if cipher_bytes.len() == 16 {
            feedback.copy_from_slice(cipher_bytes);
        }
        out.extend_from_slice(&produced);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::snmp_ber::parse_hex;

    fn h(text: &str) -> Vec<u8> {
        parse_hex(text).unwrap()
    }

    const ENGINE: &str = "00 00 00 00 00 00 00 00 00 00 00 02";

    /// RFC 3414 A.3.1 and A.3.2.
    #[test]
    fn password_to_key_matches_rfc_3414_a3() {
        let ku = Auth::Md5.password_key(b"maplesyrup");
        assert_eq!(ku, h("9f af 32 83 88 4e 92 83 4e bc 98 47 d8 ed d9 63"));
        assert_eq!(
            Auth::Md5.localize(&ku, &h(ENGINE)),
            h("52 6f 5e ed 9f cc e2 6f 89 64 c2 93 07 87 d8 2b")
        );
        let ku = Auth::Sha1.password_key(b"maplesyrup");
        assert_eq!(
            ku,
            h("9f b5 cc 03 81 49 7b 37 93 52 89 39 ff 78 8d 5d 79 14 52 11")
        );
        assert_eq!(
            Auth::Sha1.localize(&ku, &h(ENGINE)),
            h("66 95 fe bc 92 88 e3 62 82 23 5f c7 15 1f 12 84 97 b3 8f 3f")
        );
    }

    /// RFC 3414 A.5.1 and A.5.2: the new passwords' keys, localized.
    #[test]
    fn new_keys_match_rfc_3414_a5() {
        let ku = Auth::Md5.password_key(b"newsyrup");
        assert_eq!(ku, h("01 ad d2 73 10 7c 4e 59 6b 4b 00 f8 2b 1d 42 a7"));
        assert_eq!(
            Auth::Md5.localize(&ku, &h(ENGINE)),
            h("87 02 1d 7b d9 d1 01 ba 05 ea 6e 3b f9 d9 bd 4a")
        );
        let ku = Auth::Sha1.password_key(b"newsyrup");
        assert_eq!(
            Auth::Sha1.localize(&ku, &h(ENGINE)),
            h("78 e2 dc ce 79 d5 94 03 b5 8c 1b ba a5 bf f4 63 91 f1 cd 25")
        );
    }

    /// draft-blumenthal-aes-usm-04 A.1: the SHA key of A.3.2 extended to
    /// 256 bits.
    #[test]
    fn short_keys_extend_as_the_aes_draft_shows() {
        let kul = h("6695febc 9288e362 82235fc7 151f1284 97b38f3f");
        assert_eq!(
            Privacy::Aes256.key(Auth::Sha1, &kul),
            h("6695febc 9288e362 82235fc7 151f1284 97b38f3f 505e07eb 9af25568 fa1f5dbe")
        );
        assert_eq!(Privacy::Aes128.key(Auth::Sha1, &kul), kul[..16].to_vec());
    }

    /// RFC 2202 §2 and §3 (HMAC-MD5, HMAC-SHA-1) and RFC 4231 §4.2 test case
    /// 1 (HMAC-SHA-224 to -512), compared on the truncated length.
    #[test]
    fn hmac_matches_rfc_2202_and_4231() {
        let key = [0x0b; 16];
        assert_eq!(
            Auth::Md5.mac(&key, b"Hi There"),
            h("9294727a3638bb1c13f48ef8")[..12]
        );
        let key = [0x0b; 20];
        assert_eq!(
            Auth::Sha1.mac(&key, b"Hi There"),
            h("b617318655057264e28bc0b6")[..12]
        );
        assert_eq!(
            Auth::Sha224.mac(&key, b"Hi There"),
            h("896fb1128abbdf196832107cd49df33f")
        );
        assert_eq!(
            Auth::Sha256.mac(&key, b"Hi There"),
            h("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da7")
        );
        assert_eq!(
            Auth::Sha384.mac(&key, b"Hi There"),
            h("afd03944d84895626b0825f4ab46907f15f9dadbe4101ec682aa034c7cebc59c")
        );
        assert_eq!(
            Auth::Sha512.mac(&key, b"Hi There"),
            h(
                "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cde\
               daa833b7d6b8a702038b274eaea3f4e4"
            )
        );
        // RFC 2202 test 6: a key longer than the block is hashed first.
        assert_eq!(
            Auth::Sha1.mac(
                &[0xaa; 80],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            ),
            h("aa4ae5e15272d00e95705637")
        );
    }

    /// NIST SP 800-38A F.3.13 and F.3.17: CFB128-AES128 and -AES256.
    #[test]
    fn aes_cfb_matches_sp_800_38a() {
        let iv: [u8; 16] = h("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
        let plain = h("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
        let key = h("2b7e151628aed2a6abf7158809cf4f3c");
        let cipher = h("3b3fd92eb72dad20333449f8e83cfb4ac8a64537a0b3a93fcde3cdad9f1ce58b");
        assert_eq!(aes_cfb(&key, &iv, &plain, true), cipher);
        assert_eq!(aes_cfb(&key, &iv, &cipher, false), plain);
        let key = h("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let cipher = h("dc7e84bfda79164b7ecd8486985d386039ffed143b28b1c832113c6331e5407b");
        assert_eq!(aes_cfb(&key, &iv, &plain, true), cipher);
        // A last segment shorter than a block uses only its share.
        assert_eq!(
            aes_cfb(&key, &iv, &plain[..20], true),
            cipher[..20].to_vec()
        );
        assert_eq!(
            aes_cfb(&key, &iv, &cipher[..20], false),
            plain[..20].to_vec()
        );
    }

    #[test]
    fn privacy_round_trips_with_each_cipher() {
        let kul = Auth::Sha256.localize(&Auth::Sha256.password_key(b"privacy pass"), &h(ENGINE));
        let plain = b"a scoped PDU of 37 bytes, give or take";
        for p in [
            Privacy::Des,
            Privacy::Aes128,
            Privacy::Aes192,
            Privacy::Aes256,
        ] {
            let key = p.key(Auth::Sha256, &kul);
            let (data, params) = p.encrypt(&key, 3, 1000, 0x0102_0304_0506_0708, plain);
            assert_ne!(&data[..plain.len()], &plain[..]);
            let back = p.decrypt(&key, 3, 1000, &params, &data).unwrap();
            assert_eq!(&back[..plain.len()], &plain[..], "{p:?}");
        }
        // DES's salt is engineBoots then the local integer.
        let key = Privacy::Des.key(Auth::Md5, &kul[..16]);
        let (_, params) = Privacy::Des.encrypt(&key, 3, 0, 0x0506_0708, b"x");
        assert_eq!(params, vec![0, 0, 0, 3, 5, 6, 7, 8]);
    }
}
