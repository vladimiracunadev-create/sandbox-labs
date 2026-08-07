//! Dominio de mercado de capitales: dinero exacto y libro mayor.
//!
//! # Por qué es un crate aparte
//!
//! No comparte nada con el núcleo de sandboxes: ni políticas, ni runtimes, ni
//! evidencia de ejecución. Mezclarlos haría que un cambio en el aislamiento
//! recompilara el ledger y al revés, y sobre todo confundiría dos modelos de
//! amenazas distintos. Lo que sí comparten es la regla del proyecto: nada se
//! declara sin poder comprobarse.
//!
//! # Avisos que no son decorativos
//!
//! - **Dinero simulado.** Sin conexión a ningún banco, medio de pago ni cuenta
//!   real.
//! - **Sin autorización de nadie.** Un simulador no es un sandbox regulatorio, y
//!   este proyecto no representa a ninguna autoridad.
//! - **No es asesoría financiera.** Nada de lo que salga de aquí es una
//!   recomendación de inversión.

pub mod ledger;
pub mod money;

pub use ledger::{Entry, Ledger, LedgerError, Transaction};
pub use money::{Currency, Money, MoneyError};
