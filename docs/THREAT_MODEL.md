# Modelo de amenazas

## Activos

- Host, kernel y credenciales.
- Archivos fuera del workspace.
- Red local y servicios cloud.
- Disponibilidad de CPU, RAM, procesos y disco.
- Integridad de evidencias.

## Adversario

Código registrado que puede contener errores, comportamiento generado por IA o intentos controlados de salir del workspace. El proyecto no autoriza análisis de malware real en el host.

## Riesgos principales

- Escape del runtime.
- Acceso a archivos o secretos heredados.
- Egress de red.
- Agotamiento de recursos.
- Confusión entre control solicitado y control efectivo.
- Manipulación del panel local mediante CSRF, path traversal o symlinks.

## Mitigaciones actuales

- Bind en `127.0.0.1`.
- Header de escritura y validación de Origin.
- Workloads, policies y runtimes registrados.
- Rechazo de comandos libres.
- Resolución segura de archivos estáticos, incluida salida por symlink.
- Entorno limpio al ejecutar procesos.
- Timeout y límite de stdout/stderr.
- Hash SHA-256 de policy, workload y runner.
- Fail-closed para policies strict.

## Fuera de alcance actual

- Garantía formal contra escapes de kernel.
- Egress allowlist robusto.
- Multi-tenancy hostil.
- Firma de artefactos y evidencias.
- Captura completa de syscalls y eventos eBPF.
