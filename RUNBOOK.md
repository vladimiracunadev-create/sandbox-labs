# Runbook

## Panel no inicia

```bash
node scripts/validate-config.mjs
cd control-center
node scripts/build.mjs
node dist/server.js
```

## Trabajo queda `planned`

El Control Center no encontró `sandboxctl`. Compila:

```bash
cargo build -p sandboxctl --release --locked
```

O define `SANDBOXCTL_BIN` con una ruta ejecutable.

## Policy strict bloquea

Consulta `unsupported` en la evidencia. Cambia el runtime o implementa el control; no conviertas la policy a best-effort solo para evitar el error.

## Un sandbox se quedó corriendo

Un servicio **no muere con el CLI que lo levantó** — eso se quitó a propósito
para que sobreviva a `service up`—, así que puede quedarse vivo si su registro
desaparece. Ha pasado: tres sandboxes del caso 03 estuvieron cuatro horas
corriendo después de que un borrado se llevara sus registros.

Síntoma típico: `service up` falla con

```text
Error: El puerto 8803 ya está ocupado por otro proceso.
```

La salida es una, y encuentra también los que no tienen registro:

```bash
cargo run -p sandboxctl -- service down --all
```

Busca por línea de comandos las **dos** clases de huérfano —el sandbox y el
reenviador del puerto, que son procesos distintos— y los nombra antes de
detenerlos:

```text
⚠ 3 sandbox(es) vivos sin registro que los nombre:
   · PID 1289 · 03-file-detonation
   · PID 1290 · reenviador de file-detonation
```

Para mirar sin tocar nada:

```bash
ps -eo pid,etime,args | grep -E "bwrap.*sandbox-labs|service forward"
```

## Limpiar estado local

```bash
cargo run -p sandboxctl -- service down --all   # primero, siempre
node scripts/cleanup-test-state.mjs
```

El script **se niega** si hay servicios con registro, y dice cuáles. Borrar
`.sandbox-data` con un servicio en marcha destruye el registro que lo nombra, y
entonces nada del CLI vuelve a encontrarlo.

## Incidente

Detén el panel, conserva evidencia y logs, destruye la VM de pruebas y sigue [SECURITY.md](SECURITY.md).
