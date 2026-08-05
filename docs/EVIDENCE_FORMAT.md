# Formato de evidencia

Cada ejecución escribe `evidence/runs/<runId>.json`.

Campos principales:

- `runtime`: ID, versión y versión del adaptador.
- `host`: sistema, arquitectura y kernel.
- `integrity`: SHA-256 de policy, árbol del workload y runner.
- `policy.requestedControls`: controles obligatorios.
- `policy.effective`: controles realmente aplicados por el adaptador.
- `unsupported`: controles solicitados que no pudieron demostrarse.
- `limits`: límites declarados.
- `result`: código de salida, motivo, duración y salida truncada.
- `violations`: observaciones estructuradas.
- `plan`: decisiones del adaptador.

Una evidencia `planned` no demuestra aislamiento. Una evidencia `completed` tampoco prueba por sí sola la ausencia de escape; debe acompañarse de pruebas negativas.
