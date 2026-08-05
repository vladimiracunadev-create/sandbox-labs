# Primeros pasos

## Requisitos base

- Node.js 22 o superior.
- `pnpm` 9 mediante Corepack.
- Rust 1.78 o superior.
- Linux/WSL2 para Bubblewrap y namespaces.

## Preparación

```bash
corepack enable
pnpm install --frozen-lockfile
bash scripts/generate-lockfiles.sh
cargo build --workspace --locked
make check
```

## Inspección del host

```bash
cargo run -p sandboxctl -- doctor
bash scripts/doctor.sh
```

## Primera evidencia

```bash
cargo run -p sandboxctl -- plan \
  --workload workloads/benign/hello \
  --runtime dry-run \
  --policy policies/minimal.json
```

La evidencia queda en `evidence/runs/`.

## Primera ejecución real

Usa una VM de desarrollo. Para native, solo workload benigno y policy `best-effort`:

```bash
SANDBOX_LABS_ALLOW_NATIVE=1 cargo run -p sandboxctl -- run \
  --workload workloads/benign/hello \
  --runtime native \
  --policy policies/web-application.json
```

Nunca uses native con una carga desconocida.
