//! Firma local de la evidencia con Ed25519.
//!
//! # Qué añade sobre la huella
//!
//! La huella (`integrity.evidenceSha256`) detecta que el fichero se ha tocado.
//! No detecta que alguien lo haya **rehecho**: quien pueda editar el JSON puede
//! recalcular el SHA-256 y dejarlo coherente.
//!
//! Una firma cierra esa puerta a quien no tenga la clave privada. La clave vive
//! en el directorio de datos del proyecto, fuera del repositorio, con permisos
//! de solo su dueño.
//!
//! # Qué NO añade, dicho con todas sus letras
//!
//! Esta clave la genera y la guarda la misma máquina que ejecuta. Quien tenga
//! acceso al equipo tiene la clave, y con ella puede firmar lo que quiera. **No
//! es una notarización**: no prueba a un tercero que la ejecución ocurrió, prueba
//! que el informe no cambió después de escribirse sin pasar por la clave.
//!
//! Para lo primero haría falta que firmara algo que el operador no controle —un
//! runner de CI con clave efímera, un servicio de sellado— y eso es otro
//! problema, con otro modelo de amenazas. Está anotado en el backlog.

use crate::hash::to_hex;
use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use std::path::{Path, PathBuf};

/// Nombre del algoritmo tal y como aparece en la evidencia.
pub const ALGORITHM: &str = "ed25519";

/// Dónde vive la clave dentro del directorio de datos.
pub fn key_path(data_root: &Path) -> PathBuf {
    data_root.join("keys").join("evidence.ed25519")
}

/// Carga la clave de firma, generándola la primera vez.
///
/// Se genera sola y no se pide al usuario que la cree porque una firma que hay
/// que configurar antes de usar es una firma que nadie usa. Lo que sí se hace es
/// decir en la evidencia **de quién** es, con su huella pública.
pub fn load_or_create(data_root: &Path) -> Result<SigningKey> {
    let path = key_path(data_root);
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut material = [0_u8; 32];
            material.copy_from_slice(&bytes);
            return Ok(SigningKey::from_bytes(&material));
        }
        // Una clave del tamaño equivocado no se «arregla» silenciosamente: se
        // dice, porque significa que el fichero está corrupto o no es una clave.
        anyhow::bail!("La clave de firma en {} no mide 32 bytes ({} bytes)", path.display(), bytes.len());
    }

    let mut material = [0_u8; 32];
    getrandom::fill(&mut material).context("No se pudo obtener aleatoriedad del sistema para la clave")?;
    let key = SigningKey::from_bytes(&material);

    crate::dirs::ensure(path.parent().expect("directorio de claves"))?;
    std::fs::write(&path, key.to_bytes())?;
    restrict(&path)?;
    Ok(key)
}

/// Deja la clave legible solo por su dueño.
///
/// Una clave privada con permisos de lectura para todo el mundo no es una clave
/// privada. En sistemas sin permisos POSIX no se puede garantizar, y entonces la
/// firma vale lo que valga el control de acceso del sistema de ficheros.
#[cfg(unix)]
fn restrict(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict(_path: &Path) -> Result<()> {
    Ok(())
}

/// Huella pública de la clave: SHA-256 de su parte pública, en hexadecimal.
///
/// Es lo que permite decir «esta evidencia y aquella las firmó la misma clave»
/// sin publicar nada sensible.
pub fn fingerprint(key: &VerifyingKey) -> String {
    crate::hash::sha256_hex(key.as_bytes())[..32].to_string()
}

/// Firma unos bytes y devuelve (clave pública, firma), ambas en hexadecimal.
pub fn sign(key: &SigningKey, message: &[u8]) -> (String, String) {
    let signature: Signature = key.sign(message);
    (to_hex(key.verifying_key().as_bytes()), to_hex(&signature.to_bytes()))
}

/// Comprueba una firma. Cualquier dato mal formado es un fallo de verificación,
/// no un error del programa: una evidencia con una firma ilegible **no está
/// verificada**, y tratarlo de otro modo sería aprobarla por accidente.
pub fn verify(public_key: &str, signature: &str, message: &[u8]) -> Result<(), String> {
    let key_bytes = from_hex(public_key, 32).ok_or_else(|| "clave pública ilegible".to_string())?;
    let signature_bytes = from_hex(signature, 64).ok_or_else(|| "firma ilegible".to_string())?;

    let mut key_array = [0_u8; 32];
    key_array.copy_from_slice(&key_bytes);
    let mut signature_array = [0_u8; 64];
    signature_array.copy_from_slice(&signature_bytes);

    let key = VerifyingKey::from_bytes(&key_array).map_err(|_| "clave pública inválida".to_string())?;
    key.verify(message, &Signature::from_bytes(&signature_array)).map_err(|_| "la firma no corresponde".to_string())
}

fn from_hex(value: &str, expected: usize) -> Option<Vec<u8>> {
    if value.len() != expected * 2 {
        return None;
    }
    (0..value.len()).step_by(2).map(|index| u8::from_str_radix(value.get(index..index + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7_u8; 32])
    }

    #[test]
    fn a_signature_verifies_against_its_own_message() {
        let (public, signature) = sign(&key(), b"la evidencia");
        assert_eq!(verify(&public, &signature, b"la evidencia"), Ok(()));
    }

    #[test]
    fn a_changed_message_breaks_the_signature() {
        // Es el caso que importa: rehacer el JSON y recalcular su SHA-256 deja
        // la huella coherente, pero no la firma.
        let (public, signature) = sign(&key(), b"la evidencia");
        assert!(verify(&public, &signature, b"la evidencia falsificada").is_err());
    }

    #[test]
    fn another_key_does_not_pass() {
        let (_, signature) = sign(&key(), b"mensaje");
        let other = SigningKey::from_bytes(&[9_u8; 32]);
        let public = to_hex(other.verifying_key().as_bytes());
        assert!(verify(&public, &signature, b"mensaje").is_err());
    }

    #[test]
    fn malformed_data_fails_verification_instead_of_panicking() {
        // Una evidencia con la firma truncada NO está verificada. Tratar el dato
        // ilegible como error del programa la dejaría sin comprobar.
        let long_key = "a".repeat(64);
        let long_signature = "b".repeat(128);
        for (public, signature) in [("", ""), ("zz", "zz"), ("ab", "cd"), (long_key.as_str(), long_signature.as_str())]
        {
            assert!(verify(public, signature, b"mensaje").is_err(), "«{public}»/«{signature}» no puede aprobar");
        }
    }

    #[test]
    fn the_fingerprint_is_stable_and_says_nothing_secret() {
        let first = fingerprint(&key().verifying_key());
        assert_eq!(first, fingerprint(&key().verifying_key()));
        assert_eq!(first.len(), 32);
        assert_ne!(first, fingerprint(&SigningKey::from_bytes(&[9_u8; 32]).verifying_key()));
    }

    #[test]
    fn the_key_is_created_once_and_reused() {
        let directory = std::env::temp_dir().join(format!("sandbox-labs-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let first = load_or_create(&directory).expect("crear");
        let second = load_or_create(&directory).expect("reutilizar");
        assert_eq!(first.to_bytes(), second.to_bytes(), "regenerarla dejaría sin verificar todo lo firmado antes");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    #[cfg(unix)]
    fn the_key_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let directory = std::env::temp_dir().join(format!("sandbox-labs-perm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        load_or_create(&directory).expect("crear");
        let mode = std::fs::metadata(key_path(&directory)).expect("metadatos").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "una clave privada legible por todos no es privada");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
