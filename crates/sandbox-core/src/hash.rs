//! Hashes de integridad.
//!
//! Toda la evidencia se firma con SHA-256 en hexadecimal minúscula. La
//! codificación vive aquí y no en cada llamador porque `sha2` 0.11 dejó de
//! implementar `LowerHex` sobre la salida del digest: sin este módulo, un
//! `format!("{:x}", …)` disperso por el código convierte una actualización de
//! dependencia en una migración a mano.

use sha2::{Digest, Sha256};

/// Codifica bytes como hexadecimal minúscula.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // `write!` sobre un String no puede fallar, así que se evita el Result.
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("nibble válido"));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).expect("nibble válido"));
    }
    out
}

/// SHA-256 de un bloque de bytes, en hexadecimal.
pub fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    to_hex(Sha256::digest(bytes.as_ref()).as_slice())
}

/// Cierra un digest incremental y devuelve su hexadecimal.
pub fn finish_hex(digest: Sha256) -> String {
    to_hex(digest.finalize().as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_every_byte_as_two_lowercase_digits() {
        assert_eq!(to_hex(&[]), "");
        assert_eq!(to_hex(&[0x00]), "00");
        assert_eq!(to_hex(&[0x0f]), "0f");
        assert_eq!(to_hex(&[0xff]), "ff");
        assert_eq!(to_hex(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        let all: Vec<u8> = (0..=255u8).collect();
        assert_eq!(to_hex(&all).len(), 512);
        assert!(to_hex(&all).chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn matches_the_known_sha256_vector() {
        // Vector del NIST: SHA-256 de la cadena vacía.
        assert_eq!(sha256_hex(""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn incremental_and_one_shot_agree() {
        let mut digest = Sha256::new();
        digest.update(b"abc");
        assert_eq!(finish_hex(digest), sha256_hex("abc"));
    }
}
