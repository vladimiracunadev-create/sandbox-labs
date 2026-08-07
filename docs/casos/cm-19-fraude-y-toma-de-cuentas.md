# CM-19 · Fraude y toma de cuentas

> **En una frase, para cualquiera:** alguien entra en tu cuenta con tu
> contraseña correcta. El sistema no tiene forma de saber que no eres tú, salvo
> mirando si lo que hace se parece a lo que tú haces.

**Estado real:** 🔴 `planned` · **Carpeta prevista:** `domains/capital-markets/cases/19-fraud-account-takeover`

> [!WARNING]
> **Cuentas, dispositivos y sesiones simuladas. Sin datos personales reales.** No
> es una autorización regulatoria.

---

## Por qué se realiza este caso

En una toma de cuenta, la autenticación **funcionó correctamente**: quien entra
tiene las credenciales. Todos los controles de acceso dicen que sí. La única
señal disponible es el **comportamiento**.

| Señal | Por qué importa |
|---|---|
| **Dispositivo nuevo** | Primera vez que se ve, justo antes de un retiro |
| **Retiro anómalo** | Monto o destino que no encaja con el historial |
| **Cambio de beneficiario** seguido de retiro | La secuencia clásica |
| **Sesión imposible** | Dos accesos desde lugares incompatibles en el tiempo |
| Automatización | Ritmo de interacción que no es humano |
| Múltiples cuentas | El mismo dispositivo operando muchas cuentas ajenas |
| Credenciales comprometidas | Aparecen en una filtración conocida |

Igual que en [CM-15](cm-15-kyc-aml-y-sanciones.md), **el falso positivo hace
daño**: bloquear la cuenta de alguien que está de viaje y necesita su dinero es
una consecuencia real.

## La idea que enseña, y que ningún otro caso enseña

**Autenticar y autorizar no son lo mismo, y la confianza se recalcula.** El acceso
válido no concede permiso para todo: cada acción sensible se evalúa por su propio
riesgo en ese momento. Y la respuesta se **gradúa** —pedir confirmación adicional,
retrasar la operación, limitar el monto— en vez de elegir entre dejar pasar y
bloquear.

El retraso es la medida más subestimada: **una hora de espera en un cambio de
beneficiario no molesta a un cliente legítimo y arruina un fraude**.

## Casos de uso reales

- Una plataforma de inversión con retiros a cuentas bancarias.
- Una billetera con transferencias entre usuarios.
- Detección de cuentas mula.
- Formación: por qué el segundo factor no cierra el problema.

## Cómo funcionará

```mermaid
flowchart LR
  S["🔐 Sesión<br/>autenticada"] --> B["📊 Comportamiento"]
  D["📱 Dispositivo"] --> B
  H["🗂️ Historial"] --> B
  B --> R["🎚️ Riesgo de ESTA acción"]
  R --> A{"⚖️ Respuesta graduada"}
  A --> A1["✅ Permitir"]
  A --> A2["🔐 Confirmación adicional"]
  A --> A3["⏳ Retrasar"]
  A --> A4["🚫 Bloquear + revisión humana"]
  A1 & A2 & A3 & A4 --> L["📒 Registro"]
```

```mermaid
sequenceDiagram
  participant A as Atacante
  participant S as Sistema
  A->>S: acceso con credenciales correctas
  S->>S: dispositivo nunca visto → riesgo medio
  A->>S: cambiar beneficiario
  S->>S: cambio + dispositivo nuevo → riesgo ALTO
  S-->>A: ⏳ cambio aplicado con espera de 24 h
  A->>S: retirar todo
  S->>S: retiro anómalo + beneficiario reciente → riesgo CRÍTICO
  S-->>A: 🚫 bloqueado, revisión humana
```

## Esquemas

```json
{
  "event": {
    "account": "cta-sintetica-1",
    "action": "withdraw",
    "amount": { "minorUnits": 4500000, "currency": "CLP" },
    "device": { "id": "dev-nuevo-1", "firstSeen": true },
    "session": { "geoLabel": "zona-B", "previousGeoLabel": "zona-A", "minutesSincePrevious": 12 },
    "beneficiaryAgeHours": 2
  }
}
```

```json
{
  "assessment": {
    "riskScore": 0.91,
    "signals": ["NewDevice", "ImpossibleSession", "RecentBeneficiary", "AmountAnomaly"],
    "response": "block-and-review",
    "humanReviewRequired": true,
    "falsePositiveCost": "el cliente no puede retirar hasta la revisión"
  }
}
```

`falsePositiveCost` está en el esquema **a propósito**: obliga a escribir lo que
cuesta equivocarse, junto a la decisión de bloquear.

## Software necesario

| Componente | Para qué |
|---|---|
| **Rust** 1.75+ | Señales, puntuación de riesgo y respuesta graduada |
| **Node.js** 20+ / **pnpm** 9+ | Cola de revisión humana (recomendado) |

Sin jaula ni Linux. **Cuentas, dispositivos y ubicaciones son sintéticos**; las
ubicaciones se representan como etiquetas, no como coordenadas, para no modelar
datos personales ni siquiera de forma simulada.

## Instalación

```bash
cargo build --release
```

## Procesos que se crearán

```text
sandboxctl markets fraud --scenario toma-de-cuenta
  │
  └─ un proceso determinista, sin red
      ├─ historial sintético por cuenta
      ├─ evaluación por acción, no por sesión
      └─ cola de revisión humana
```

## Tiempo de carga estimado

| Operación | Coste esperado |
|---|---|
| Evaluar una acción | < 1 ms |
| Simular 100 000 eventos de sesión | segundos |
| Reconstruir el historial de una cuenta | milisegundos |

## Qué hace falta para construirlo

1. Historial de comportamiento por cuenta, sintético.
2. Las siete señales listadas, cada una con escenario.
3. Puntuación de riesgo **por acción**, no por sesión.
4. Respuesta graduada, incluido el retraso como medida de primera línea.
5. Métrica de falsos positivos junto a la de detección.
6. Cola de revisión humana antes de cualquier bloqueo prolongado.

---

**Ver también:** [Catálogo completo](../CATALOGO.md) · [CM-15 · KYC y AML](cm-15-kyc-aml-y-sanciones.md) · [CM-11 · consentimiento](cm-11-finanzas-abiertas-y-consentimiento.md)
