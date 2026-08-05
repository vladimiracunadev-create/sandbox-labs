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

## Limpiar estado local

```bash
rm -rf .sandbox-data
afind=evidence/runs
find "$afind" -type f -name '*.json' -delete
```

## Incidente

Detén el panel, conserva evidencia y logs, destruye la VM de pruebas y sigue [SECURITY.md](SECURITY.md).
