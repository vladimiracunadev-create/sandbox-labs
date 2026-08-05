# Security Policy

## Advertencia

Este proyecto es educativo y experimental. No ejecutes malware ni código desconocido en el host. Usa una VM o infraestructura descartable y aplica defensa en profundidad.

## Reporte responsable

No publiques detalles de un escape confirmado en un issue público. Contacta al mantenedor por un canal privado y entrega:

- runtime y versión;
- host/kernel;
- policy y workload mínimos;
- evidencia JSON;
- impacto y pasos de reproducción seguros.

## Principios

- Fail-closed para policies strict.
- Cero comandos arbitrarios por HTTP.
- Bind local por defecto.
- Entorno limpio, rutas canónicas y symlinks rechazados.
- Límites de tiempo y salida.
- Workloads riesgosos bloqueados en native.
- Estados honestos: installed no equivale a secure.

## No cubierto

No se garantiza aislamiento contra vulnerabilidades del kernel, hipervisor o runtime. La seguridad depende del host y de la configuración efectiva registrada en evidencia.
