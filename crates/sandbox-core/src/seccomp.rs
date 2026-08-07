//! Filtros seccomp: la única forma de que `policy.syscalls` signifique algo.
//!
//! # Qué había antes
//!
//! `profiles/seccomp/strict.json` existía, `policy.syscalls` se parseaba, y
//! ningún adaptador compilaba nada ni se lo pasaba al runtime. Un fichero de
//! perfil que nadie aplica sugiere una capacidad que el sistema no tiene, que es
//! peor que no tenerlo.
//!
//! # Lista de denegación, no de permitidos
//!
//! Las políticas del catálogo declaran `syscalls.deny`, así que eso es lo que se
//! compila: permitir por defecto y devolver `EPERM` en las llamadas nombradas.
//!
//! Una lista de permitidos contiene mucho más —cualquier llamada no prevista
//! muere— pero exige enumerar todo lo que un intérprete de Python necesita para
//! arrancar, que cambia entre versiones de glibc y del propio Python. Una lista
//! de permitidos incompleta no es «más segura»: es un sandbox que no arranca, y
//! el arreglo habitual es ampliarla hasta que funcione, momento en el cual ya no
//! contiene nada. Cuando exista un caso con un binario conocido y estable, la
//! lista de permitidos será lo correcto para él; para código arbitrario, no.
//!
//! # Por qué `EPERM` y no matar el proceso
//!
//! Matar deja un proceso muerto sin explicación. Con `EPERM` la carga recibe un
//! error normal, sigue viva y **puede contarlo**: la sonda de la suite de
//! contención distingue un `EPERM` del filtro de los errores que la llamada
//! daría igualmente, y eso es lo que convierte el control en algo medido en vez
//! de declarado.

use crate::Policy;
use anyhow::{Context, Result};
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule, TargetArch};
use std::collections::BTreeMap;

/// Descriptor por el que bubblewrap lee el filtro.
///
/// Un número alto y fijo: los descriptores bajos los usa el propio supervisor
/// para las tuberías de la salida, y `dup2` sobre uno de ellos las cerraría.
/// Tiene que conocerse antes de construir los argumentos, porque va en
/// `--seccomp <fd>`.
pub const FILTER_FD: i32 = 63;

/// Arquitecturas para las que se sabe compilar. En cualquier otra no se genera
/// filtro y el control no se declara: un filtro compilado para la arquitectura
/// equivocada no protege de nada.
fn target_arch() -> Option<TargetArch> {
    match std::env::consts::ARCH {
        "x86_64" => Some(TargetArch::x86_64),
        "aarch64" => Some(TargetArch::aarch64),
        _ => None,
    }
}

/// ¿Puede este host respaldar el control `syscalls` para esta política?
///
/// Responde a las tres condiciones a la vez: hay algo que denegar, la
/// arquitectura se conoce, y el filtro compila. Lo usan tanto el planificador
/// —para decidir si declara el control— como el adaptador, para no divergir.
pub fn is_supported(policy: &Policy) -> bool {
    compile(policy).map(|filter| filter.is_some()).unwrap_or(false)
}

/// Compila `policy.syscalls.deny` a un programa BPF.
///
/// `Ok(None)` significa «no hay filtro que aplicar»: la política no deniega
/// nada, o la arquitectura no se conoce. No es un error, pero tampoco un
/// control.
pub fn compile(policy: &Policy) -> Result<Option<BpfProgram>> {
    if policy.syscalls.deny.is_empty() {
        return Ok(None);
    }
    let Some(arch) = target_arch() else {
        return Ok(None);
    };

    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    let mut known = 0_usize;
    for name in &policy.syscalls.deny {
        // Un nombre que este kernel no conoce se ignora en vez de tumbar la
        // compilación: `clone3` no existe en kernels antiguos, y que la política
        // lo nombre no debería impedir que el resto del filtro se aplique.
        if let Some(number) = syscall_number(name) {
            // Vec vacío = la llamada se filtra siempre, sin condiciones sobre
            // sus argumentos.
            rules.insert(number, vec![]);
            known += 1;
        }
    }
    if known == 0 {
        return Ok(None);
    }

    let filter = SeccompFilter::new(
        rules,
        // Acción por defecto: dejar pasar. Lo que se deniega está en las reglas.
        SeccompAction::Allow,
        // Acción de la regla: error, no muerte. Ver la nota del módulo.
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )
    .context("No se pudo construir el filtro seccomp")?;

    let program: BpfProgram = filter.try_into().context("No se pudo compilar el filtro seccomp a BPF")?;
    Ok(Some(program))
}

/// El programa BPF en la forma que bubblewrap espera leer del descriptor: una
/// secuencia de `struct sock_filter`, ocho bytes cada una, en el orden de bytes
/// nativo.
pub fn to_bytes(program: &BpfProgram) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(program.len() * 8);
    for instruction in program {
        bytes.extend_from_slice(&instruction.code.to_ne_bytes());
        bytes.push(instruction.jt);
        bytes.push(instruction.jf);
        bytes.extend_from_slice(&instruction.k.to_ne_bytes());
    }
    bytes
}

/// Número de llamada al sistema por nombre, para las que las políticas del
/// catálogo denegan.
///
/// Una tabla explícita y no una biblioteca: son siete nombres, cambian muy poco,
/// y una tabla que se lee es más auditable que una dependencia que hay que
/// creerse. Los números son los de Linux para cada arquitectura — **no**
/// coinciden entre ellas, que es justo el error que un `match` compartido
/// escondería.
fn syscall_number(name: &str) -> Option<i64> {
    #[cfg(target_arch = "x86_64")]
    let number = match name {
        "mount" => 165,
        "umount2" => 166,
        "ptrace" => 101,
        "reboot" => 169,
        "kexec_load" => 246,
        "bpf" => 321,
        "perf_event_open" => 298,
        "clone3" => 435,
        "init_module" => 175,
        "finit_module" => 313,
        "delete_module" => 176,
        "process_vm_readv" => 310,
        "process_vm_writev" => 311,
        // Calibración de la suite de contención: siempre tiene éxito para
        // cualquier usuario, así que un EPERM solo puede venir del filtro.
        "getcpu" => 309,
        _ => return None,
    };
    #[cfg(target_arch = "aarch64")]
    let number = match name {
        "mount" => 40,
        "umount2" => 39,
        "ptrace" => 117,
        "reboot" => 142,
        "kexec_load" => 104,
        "bpf" => 280,
        "perf_event_open" => 241,
        "clone3" => 435,
        "init_module" => 105,
        "finit_module" => 273,
        "delete_module" => 106,
        "process_vm_readv" => 270,
        "process_vm_writev" => 271,
        "getcpu" => 168,
        _ => return None,
    };
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let number = {
        let _ = name;
        return None;
    };
    Some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_denying(deny: &[&str]) -> Policy {
        serde_json::from_value(serde_json::json!({
            "id": "test",
            "enforcement": { "mode": "best-effort", "requiredControls": [] },
            "filesystem": { "root": "ephemeral", "readOnly": [], "writable": [], "maxWorkspaceMb": 64, "followSymlinks": false },
            "network": { "mode": "none", "hosts": [], "dns": "disabled" },
            "resources": { "cpu": 1.0, "memoryMb": 128, "processes": 8, "timeoutSeconds": 10, "openFiles": 32, "outputBytes": 4096 },
            "process": { "capabilities": [], "environment": {}, "allowedEnvironment": [], "user": 65534, "group": 65534 },
            "syscalls": { "profile": "strict", "allow": [], "deny": deny }
        }))
        .expect("política de prueba válida")
    }

    #[test]
    fn a_policy_that_denies_nothing_produces_no_filter() {
        // Y por tanto no puede declarar el control. Es el caso de la mayoría de
        // las políticas de servicio.
        assert!(compile(&policy_denying(&[])).expect("compilar").is_none());
        assert!(!is_supported(&policy_denying(&[])));
    }

    #[test]
    fn the_catalogue_deny_list_compiles() {
        // La misma lista que `policies/containment-audit.json`, calibración incluida.
        let program =
            compile(&policy_denying(&["mount", "ptrace", "reboot", "kexec_load", "bpf", "perf_event_open", "getcpu"]))
                .expect("compilar")
                .expect("con llamadas denegadas tiene que haber filtro");
        assert!(!program.is_empty(), "un filtro vacío no filtra nada");
        assert!(is_supported(&policy_denying(&["ptrace"])));
        // Gancho para comprobar a mano que bubblewrap acepta este formato de
        // bytes, que es lo único que no puede verificarse sin bwrap instalado.
        if let Some(path) = std::env::var_os("SANDBOX_LABS_EMIT_BPF") {
            std::fs::write(path, to_bytes(&program)).expect("volcar el filtro");
        }
    }

    #[test]
    fn an_unknown_name_does_not_sink_the_rest_of_the_filter() {
        // `clone3` no existe en kernels antiguos y hay nombres que se escriben
        // mal. Que uno no se reconozca no puede dejar la carga sin filtro.
        let program = compile(&policy_denying(&["ptrace", "no_existe_esta_llamada"]))
            .expect("compilar")
            .expect("el nombre conocido tiene que seguir filtrándose");
        assert!(!program.is_empty());
    }

    #[test]
    fn only_unknown_names_produce_no_filter() {
        // Y entonces el control no se declara, en vez de declararse sobre un
        // filtro que no filtra nada.
        assert!(compile(&policy_denying(&["esto", "tampoco"])).expect("compilar").is_none());
    }

    /// `getcpu(NULL, NULL, NULL)` y su errno, o 0 si tuvo éxito.
    ///
    /// Se elige `getcpu` y no una llamada «peligrosa» porque tiene éxito para
    /// cualquier usuario en cualquier host. Las peligrosas ya fallan con `EPERM`
    /// por falta de privilegios, y entonces la comprobación aprobaría con filtro
    /// y sin él: mediría el privilegio del usuario, no el filtro.
    ///
    /// El primer intento usó `perf_event_open`, que devuelve `EFAULT` en esta
    /// máquina pero `EACCES` en el runner de CI —y `EPERM` donde
    /// `perf_event_paranoid` valga 3—. Es justo el error contra el que avisaba
    /// el comentario de esta función, cometido a pesar de él.
    #[cfg(target_arch = "x86_64")]
    fn getcpu_errno() -> i32 {
        unsafe {
            *libc::__errno_location() = 0;
            let result = libc::syscall(309, std::ptr::null::<u8>(), std::ptr::null::<u8>(), std::ptr::null::<u8>());
            if result >= 0 {
                0
            } else {
                *libc::__errno_location()
            }
        }
    }

    /// La prueba que decide si todo esto sirve para algo.
    ///
    /// Compilar un BPF y que el kernel lo acepte son cosas distintas. Esta
    /// aplica el programa a un hilo de verdad y ejecuta la llamada denegada:
    /// si el filtro no hiciera nada, la llamada seguiría teniendo éxito.
    ///
    /// El filtro se aplica en un hilo aparte porque seccomp **no se puede
    /// quitar**: dejarlo en el hilo principal filtraría el resto de la suite.
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn the_compiled_filter_really_denies_the_syscall() {
        let program = compile(&policy_denying(&["getcpu"])).expect("compilar").expect("filtro");

        assert_eq!(getcpu_errno(), 0, "sin filtro `getcpu` tiene éxito en cualquier host");

        let filtered = std::thread::spawn(move || {
            // Sin `no_new_privs` el kernel rechaza instalar un filtro a un
            // proceso sin privilegios. Es la misma condición que bubblewrap
            // cumple por su cuenta.
            unsafe {
                assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0, "no_new_privs");
            }
            seccompiler::apply_filter(&program).expect("el kernel tiene que aceptar el programa");
            getcpu_errno()
        })
        .join()
        .expect("hilo filtrado");

        assert_eq!(filtered, libc::EPERM, "con el filtro puesto la llamada tiene que devolver EPERM");
        // Y el hilo principal sigue sin filtrar: seccomp es por hilo salvo TSYNC.
        assert_eq!(getcpu_errno(), 0, "el filtro no puede haberse escapado a este hilo");
    }

    #[test]
    fn the_serialised_program_has_eight_bytes_per_instruction() {
        // Es el formato que bubblewrap lee del descriptor. Un tamaño distinto
        // haría que interpretara basura como instrucciones.
        let program = compile(&policy_denying(&["ptrace"])).expect("compilar").expect("filtro");
        assert_eq!(to_bytes(&program).len(), program.len() * 8);
    }
}
