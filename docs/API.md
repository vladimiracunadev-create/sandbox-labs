# API del Control Center

Base: `http://127.0.0.1:9093`.

## Lectura

- `GET /api/system`
- `GET /api/catalog`
- `GET /api/policies`
- `GET /api/workloads`
- `GET /api/jobs`
- `GET /api/jobs/:id`
- `GET /api/jobs/:id/events` — Server-Sent Events
- `GET /api/evidence`
- `GET /api/evidence/:runId`

## Escritura

Las operaciones requieren `X-Sandbox-Request: 1` y Origin local cuando está presente.

### Crear trabajo

```json
{
  "workloadId": "hello",
  "policyId": "minimal",
  "runtimeId": "dry-run",
  "arguments": []
}
```

`POST /api/jobs`

No existe campo `command`. Los argumentos están limitados en cantidad, longitud y caracteres de control.

### Cancelar

`POST /api/jobs/:id/cancel`
