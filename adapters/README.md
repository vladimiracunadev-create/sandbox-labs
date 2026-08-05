# Adaptadores

El código ejecutable vive en `crates/sandbox-runtimes`. Esta carpeta conserva runbooks, artefactos y decisiones específicas por runtime.

Un adaptador debe implementar `probe`, `prepare`, supervisor común y cleanup, además de declarar controles efectivos y no soportados.
