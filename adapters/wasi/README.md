# Adaptador WASI

El adaptador debe convertir `filesystem.readOnly` y `filesystem.writable` en
preopens explícitos de Wasmtime. No hereda variables de entorno salvo las
incluidas en la política.
