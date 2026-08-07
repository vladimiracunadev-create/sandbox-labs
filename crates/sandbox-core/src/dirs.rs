//! Crear directorios sobre DrvFs sin que el proyecto se caiga por ello.
//!
//! DrvFs es el montaje de un disco de Windows dentro de WSL, y es donde vive
//! este repositorio en la máquina de desarrollo. Su caché puede quedar
//! desincronizada —típicamente porque algo borró una ruta desde Windows
//! mientras WSL la tenía vista— y entonces pasan las dos cosas a la vez:
//!
//! ```text
//! mkdir .sandbox-data  → cannot create directory: File exists
//! ls -ld .sandbox-data → No such file or directory
//! ```
//!
//! `create_dir_all` devuelve `EEXIST` o `ENOENT` según el caso, y las dos son
//! mentira. Un reintento tras una pausa corta refresca la caché.
//!
//! Esto vivía duplicado en el lanzador de servicios y volvió a hacer falta al
//! guardar la clave de firma, que sin esto no llegaba a existir en la
//! plataforma objetivo del proyecto. Un sitio, no dos.

use anyhow::{bail, Result};
use std::{path::Path, thread::sleep, time::Duration};

/// Cuánto se espera antes de reintentar. Lo justo para que DrvFs se entere.
const REFRESH: Duration = Duration::from_millis(150);

/// Crea el directorio y todos sus padres, tolerando la caché de DrvFs.
pub fn ensure(path: &Path) -> Result<()> {
    if attempt(path) {
        return Ok(());
    }
    // Segundo intento tras la pausa: si el problema era la caché, aquí ya está.
    sleep(REFRESH);
    if attempt(path) {
        return Ok(());
    }
    bail!(
        "No se pudo crear {}: el sistema de archivos dice que ya existe pero no está. \
         Si el repositorio vive en /mnt/c y borraste la ruta desde Windows, cierra WSL \
         (`wsl --shutdown`) y vuelve a intentarlo.",
        path.display()
    )
}

/// Un intento. `true` si al terminar el directorio está de verdad.
fn attempt(path: &Path) -> bool {
    let _ = std::fs::create_dir_all(path);
    path.is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("sandbox-labs-dirs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn creates_the_whole_chain() {
        let root = scratch("cadena");
        let deep = root.join("uno").join("dos").join("tres");
        ensure(&deep).expect("crear");
        assert!(deep.is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn is_idempotent() {
        // Se llama en cada arranque: fallar la segunda vez lo haría inútil.
        let root = scratch("idempotente");
        ensure(&root).expect("primera");
        ensure(&root).expect("segunda");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_in_the_way_is_an_error_not_a_silent_pass() {
        // Si donde debería ir el directorio hay un fichero, no se puede seguir
        // como si nada: lo que venga después escribiría en ninguna parte.
        let root = scratch("fichero");
        std::fs::create_dir_all(root.parent().expect("padre")).ok();
        std::fs::write(&root, b"soy un fichero").expect("escribir");
        assert!(ensure(&root).is_err());
        let _ = std::fs::remove_file(&root);
    }
}
