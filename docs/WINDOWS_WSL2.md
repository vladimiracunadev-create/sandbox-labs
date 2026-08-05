# Windows y WSL2

El Control Center y la planificación funcionan en Windows. Bubblewrap, namespaces, cgroups y gVisor se ejecutan dentro de una distribución Linux WSL2 o una VM Linux.

```powershell
wsl --install -d Ubuntu
wsl -d Ubuntu -- bash -lc "sudo apt-get update && sudo apt-get install -y bubblewrap util-linux"
```

Clona el repositorio dentro del filesystem Linux para evitar diferencias de permisos y rendimiento:

```bash
cd ~
git clone <repositorio> sandbox-labs
```

Firecracker necesita KVM real y no debe asumirse disponible dentro de cualquier configuración WSL2.
