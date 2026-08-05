# Referencia de políticas

## Enforcement

- `strict`: falla cerrado si falta cualquier `requiredControl`.
- `best-effort`: ejecuta y registra controles no soportados.

Controles válidos: `filesystem`, `network`, `processes`, `memory`, `cpu`, `timeout`, `capabilities`, `syscalls`, `devices`, `environment`, `output`.

## Filesystem

- `root`: `ephemeral`, `host-readonly` o `custom`.
- `readOnly`: rutas esperadas como solo lectura.
- `writable`: rutas con escritura explícita.
- `maxWorkspaceMb`: cuota lógica; requiere implementación del runtime para considerarse efectiva.
- `followSymlinks`: debe permanecer `false` para cargas no confiables.

## Network

- `none`: sin interfaz de red utilizable.
- `loopback`: solo localhost; no todos los adaptadores lo aplican.
- `allowlist`: requiere proxy o firewall controlado; no se implementa por simple resolución DNS.
- `unrestricted`: solo para entornos descartables.

## Recursos

`cpu`, `memoryMb`, `processes`, `timeoutSeconds`, `openFiles` y `outputBytes`.

El timeout y el límite de salida son aplicados por el supervisor común. Bubblewrap puede usar `prlimit` para address space, procesos, archivos abiertos y tiempo CPU, pero esto no reemplaza cgroups v2.
