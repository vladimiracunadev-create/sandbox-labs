# 12 · Notebooks de ciencia de datos

> **En una frase, para cualquiera:** un cuaderno de análisis es un programa que
> se ejecuta trozo a trozo, con acceso a los datos de verdad, y que a menudo lo
> ejecuta alguien que no lo escribió.

**Estado real:** 🔴 `planned` — **no hay código todavía**

---

## Por qué se realiza este caso

Un notebook parece un documento y es **código arbitrario**. Se comparte por
correo, se copia de internet, se hereda de quien se fue del equipo. Y se ejecuta
sobre los datos reales, que suelen ser lo más sensible que tiene la
organización.

| Lo que pasa habitualmente | Consecuencia |
|---|---|
| El notebook escribe sobre el dataset de origen | Se corrompen los datos para todo el mundo |
| Una celda hace una petición a internet | Los datos salen sin que nadie lo decidiera |
| Se instala una dependencia desde la celda | Código nuevo, sin revisión, con acceso a todo |
| El notebook consume toda la memoria | El servidor compartido se cae para los demás |
| Los resultados se mezclan con los datos de entrada | Nadie sabe qué es original y qué es derivado |

## La idea que enseña, y que ningún otro caso enseña

**Datos de entrada de solo lectura, salida en otro sitio.** Es un control que no
aparece en ningún otro caso porque en los demás lo desconocido es el programa;
aquí lo valioso son los datos, y protegerlos consiste en que **el análisis no
pueda modificar aquello que analiza**.

Con un efecto secundario que la gente agradece más que la seguridad: se vuelve
imposible «arruinar el dataset sin querer».

## Casos de uso reales

- Un equipo de análisis que comparte notebooks sobre los mismos datos.
- Formación con datos reales anonimizados.
- Un notebook heredado que hay que ejecutar para entender qué hacía.
- Reproducir un análisis publicado.
- Un concurso de modelos donde cada participante ejecuta su código.

## Cómo funcionará

```mermaid
flowchart LR
  N["📓 Notebook"] --> J
  DS["📊 Datasets"] -->|"solo lectura"| J
  subgraph J["🔒 Jaula con cuotas"]
    K["🐍 Kernel<br/>memoria · CPU · procesos acotados"]
  end
  J -->|"escritura"| O["📁 Carpeta de salida<br/>separada"]
  J --> E["🧾 Evidencia:<br/>qué leyó, qué escribió,<br/>qué intentó"]
  J -. "red configurable" .-> W["🌐 Lista de permitidos<br/>o ninguna"]
```

```mermaid
flowchart TB
  A["Celda ejecutada"] --> B{"¿Escribe en<br/>el dataset?"}
  B -- sí --> B1["🚫 Montaje de solo lectura:<br/>falla en el sistema de ficheros"]
  B -- no --> C{"¿Sale a la red?"}
  C -- sí --> C1{"¿Hay lista<br/>de permitidos?"}
  C1 -- no --> C2["🚫 Sin red"]
  C1 -- sí --> C3["📣 Se registra el destino"]
  C -- no --> D["✅ Ejecuta con cuota"]
```

## Esquemas

### Configuración de la sesión

```json
{
  "notebook": "analisis.ipynb",
  "datasets": [{ "path": "datos/ventas.parquet", "mode": "ro" }],
  "output": { "path": "salida/", "maxBytes": 1073741824 },
  "network": "none",
  "limits": { "memoryMb": 4096, "cpuQuota": "200%", "pids": 64, "timeoutSeconds": 3600 },
  "gpu": false
}
```

### Acta de la sesión

```json
{
  "cellsExecuted": 42,
  "datasetsRead": ["datos/ventas.parquet"],
  "writeAttemptsOnReadOnly": [{ "path": "datos/ventas.parquet", "outcome": "solo lectura" }],
  "outputsProduced": ["salida/informe.png"],
  "peakMemoryMb": 2810,
  "networkAttempts": []
}
```

## Software necesario

| Componente | Para qué | ¿Obligatorio? |
|---|---|---|
| **Python** 3.11+ con Jupyter | El kernel | Sí |
| **Rust** 1.75+ | El supervisor y las cuotas | Sí |
| **`bubblewrap`** 0.6+ | Montajes de solo lectura y separación de salida | Sí |
| **`systemd`** modo usuario | `memory.max`, `cpu.max`, `pids.max` | Sí: sin cuotas, este caso no tiene sentido |
| **GPU + contenedor con drivers** | Opcional, para cargas que la necesiten | No |
| **Linux o WSL2** | Namespaces sin privilegios | Sí |

## Instalación

```bash
sudo apt install bubblewrap python3 python3-pip
pip install jupyter
cargo build --release
cargo run -p sandboxctl -- doctor
```

## Procesos que se crearán

```text
sandboxctl notebook run analisis.ipynb
  │
  ├─ systemd --user scope        ← memory.max, cpu.max, pids.max
  │   └─ bwrap                   ← datasets ro, salida rw, red según política
  │       └─ jupyter kernel
  │           └─ los procesos que lance el notebook (acotados por pids.max)
  │
  └─ sandboxctl service forward  ← solo si se quiere interfaz web
```

`pids.max` importa más de lo que parece: un notebook que hace paralelismo puede
lanzar un proceso por núcleo por celda, y sin techo eso se convierte en una bomba
de procesos sin ninguna mala intención.

## Tiempo de carga estimado

| Operación | Coste esperado |
|---|---|
| Arranque del kernel de Jupyter | 1–3 s |
| Arranque de la jaula | 5–15 ms |
| Envoltura en cgroup | 30–80 ms |
| Montar datasets de solo lectura | < 10 ms |
| Ejecución de una celda | lo que tarde, con techo global |

## Qué hace falta para construirlo

1. Montajes de solo lectura por dataset, declarados en la configuración.
2. Carpeta de salida separada, con cuota de tamaño.
3. Cuotas obligatorias de memoria, CPU y procesos.
4. Registro de intentos de escritura sobre datos de solo lectura.
5. Limpieza garantizada al terminar la sesión.

---

**Ver también:** [Catálogo completo](../CATALOGO.md) · [Referencia de políticas](../POLICY_REFERENCE.md) · [Caso 02 · código generado](02-codigo-generado-por-ia.md)
