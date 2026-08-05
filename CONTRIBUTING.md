# Contributing

1. Crea una rama descriptiva.
2. Mantén un cambio vertical: código, esquema, pruebas y documentación.
3. Usa `pnpm`; no agregues npm/yarn.
4. Ejecuta `make check` y `cargo test --workspace --locked`.
5. No marques controles como efectivos sin prueba.
6. Añade una prueba negativa para cada nueva frontera de seguridad.
7. Actualiza `CHANGELOG.md` y `PROJECT_STATUS.md`.

Los PR de runtimes deben incluir host, versión, comandos efectivos, limitaciones y evidencia anonimizada.
