# Formato de evidencia

Cada ejecución escribe `evidence/runs/<runId>.json`.

Campos principales:

- `runtime`: ID, versión y versión del adaptador.
- `host`: sistema, arquitectura y kernel.
- `integrity`: SHA-256 de policy, árbol del workload y runner.
- `policy.requestedControls`: controles obligatorios.
- `policy.effective`: controles realmente aplicados por el adaptador.
- `unsupported`: controles solicitados que no pudieron demostrarse.
- `limits.requested`: los límites que la política pidió.
- `limits.effective`: los que el runtime aplicó **de verdad** en esta ejecución.
  Solo entra aquí lo que se tradujo en un argumento real de la línea de
  comandos, y cada entrada nombra el mecanismo: `cgroup memory.max` no es lo
  mismo que `RLIMIT_AS`, y la evidencia no puede confundirlos.
- `limits.observed`: lo que la carga **consumió**, leído del cgroup mientras
  corría. Aplicar y medir son cosas distintas.

  | Campo | Del kernel | Qué responde |
  |---|---|---|
  | `memoryPeakBytes` | `memory.peak` | cuánta memoria llegó a ocupar |
  | `pidsPeak` | `pids.peak` | cuántos procesos tuvo vivos a la vez |
  | `cpuUsageUsec` | `usage_usec` de `cpu.stat` | cuánta CPU gastó |
  | `oomKills` | `oom_kill` de `memory.events` | si el kernel mató algo por memoria |

  Va vacío cuando no hubo cgroup propio del que leer. **Nunca** se rellena con
  las cifras del cgroup de la sesión del host: serían números reales de la
  máquina equivocada, que es peor que no medir.

  Un campo ausente significa «no se pudo leer», no «cero». La diferencia importa:
  cero es un hecho medido.
- `result`: código de salida, motivo, duración y salida truncada.
- `violations`: observaciones estructuradas.
- `plan`: decisiones del adaptador.

Una evidencia `planned` no demuestra aislamiento. Una evidencia `completed` tampoco prueba por sí sola la ausencia de escape; debe acompañarse de pruebas negativas.
