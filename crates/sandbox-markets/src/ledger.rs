//! Libro mayor de doble entrada, solo-añadir.
//!
//! # Las tres reglas que lo definen
//!
//! 1. **Cada transacción cuadra a cero.** Si un asiento no tiene contrapartida,
//!    el dinero apareció o desapareció, y eso no pasa: se movió de sitio.
//! 2. **Nada se borra.** Un error se corrige con una **reversa**, que es otra
//!    transacción que apunta a la original. Las dos quedan. Un libro donde se
//!    puede borrar no es un libro, es un borrador.
//! 3. **Una transacción se aplica una vez.** La clave de idempotencia hace que
//!    reintentar un pago no lo cobre dos veces, que es el fallo más caro y más
//!    fácil de tener en un sistema con reintentos.
//!
//! # Qué NO es
//!
//! Dinero simulado. No hay conexión con ningún banco, ningún medio de pago ni
//! ninguna cuenta real, y el simulador no está autorizado por nadie. Ver
//! `docs/DISCLAIMER.md`.

use crate::money::{Currency, Money, MoneyError};
use std::collections::BTreeMap;

/// Identificador de cuenta. Cadena y no entero para que un extracto se lea:
/// `cliente:ana:efectivo` dice qué es sin consultar una tabla.
pub type AccountId = String;

/// Un movimiento sobre una cuenta. El signo lleva la dirección: positivo entra,
/// negativo sale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub account: AccountId,
    pub amount: Money,
}

impl Entry {
    pub fn new(account: impl Into<AccountId>, amount: Money) -> Self {
        Self { account: account.into(), amount }
    }
}

/// Una transacción: varios asientos que, juntos, cuadran a cero por moneda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Clave de idempotencia. Dos transacciones con la misma clave son la
    /// misma, se manden las veces que se manden.
    pub id: String,
    pub description: String,
    pub entries: Vec<Entry>,
    /// Si esta transacción revierte a otra, cuál. La original nunca se toca.
    pub reverses: Option<String>,
}

impl Transaction {
    pub fn new(id: impl Into<String>, description: impl Into<String>, entries: Vec<Entry>) -> Self {
        Self { id: id.into(), description: description.into(), entries, reverses: None }
    }
}

/// Lo que puede rechazar el libro. Todo con nombre: un ledger que falla con
/// «error» no permite a nadie corregir nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// Los asientos no suman cero. Se dice cuánto sobra y en qué moneda.
    Unbalanced { currency: Currency, difference: Money },
    /// Una transacción sin asientos no mueve nada y no debería existir.
    Empty,
    /// Ya se aplicó una transacción con esa clave.
    Duplicate { id: String },
    /// Se intenta revertir algo que no está en el libro.
    UnknownReversal { id: String },
    /// Se intenta revertir dos veces lo mismo.
    AlreadyReversed { id: String },
    /// Aritmética.
    Money(MoneyError),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unbalanced { currency, difference } => write!(
                f,
                "la transacción no cuadra en {}: sobran {difference}. El dinero se mueve, no aparece",
                currency.code()
            ),
            Self::Empty => write!(f, "una transacción sin asientos no mueve nada"),
            Self::Duplicate { id } => {
                write!(f, "la transacción «{id}» ya se aplicó: aplicarla otra vez la cobraría dos veces")
            }
            Self::UnknownReversal { id } => write!(f, "no se puede revertir «{id}»: no está en el libro"),
            Self::AlreadyReversed { id } => write!(f, "«{id}» ya estaba revertida"),
            Self::Money(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<MoneyError> for LedgerError {
    fn from(value: MoneyError) -> Self {
        Self::Money(value)
    }
}

/// El libro. Guarda las transacciones en el orden en que llegaron y los saldos
/// derivados de ellas.
#[derive(Debug, Default)]
pub struct Ledger {
    /// Historia, en orden. **Nunca se quita nada de aquí.**
    journal: Vec<Transaction>,
    /// Saldo por cuenta y moneda. Es caché de la historia: se puede reconstruir
    /// entero recorriendo el diario, y una prueba lo comprueba.
    balances: BTreeMap<(AccountId, Currency), Money>,
    /// Claves ya aplicadas, para la idempotencia.
    applied: BTreeMap<String, usize>,
    /// Qué transacciones ya tienen reversa.
    reversed: BTreeMap<String, String>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Aplica una transacción. Falla en cerrado ante cualquier duda.
    pub fn post(&mut self, transaction: Transaction) -> Result<(), LedgerError> {
        if transaction.entries.is_empty() {
            return Err(LedgerError::Empty);
        }
        if self.applied.contains_key(&transaction.id) {
            return Err(LedgerError::Duplicate { id: transaction.id });
        }
        if let Some(target) = &transaction.reverses {
            if !self.applied.contains_key(target) {
                return Err(LedgerError::UnknownReversal { id: target.clone() });
            }
            if let Some(existing) = self.reversed.get(target) {
                let _ = existing;
                return Err(LedgerError::AlreadyReversed { id: target.clone() });
            }
        }
        self.check_balanced(&transaction.entries)?;

        for entry in &transaction.entries {
            let key = (entry.account.clone(), entry.amount.currency());
            let current = self.balances.get(&key).copied().unwrap_or_else(|| Money::zero(entry.amount.currency()));
            self.balances.insert(key, current.checked_add(entry.amount)?);
        }
        if let Some(target) = &transaction.reverses {
            self.reversed.insert(target.clone(), transaction.id.clone());
        }
        self.applied.insert(transaction.id.clone(), self.journal.len());
        self.journal.push(transaction);
        Ok(())
    }

    /// Construye y aplica la reversa de una transacción: los mismos asientos con
    /// el signo cambiado.
    ///
    /// La original **se queda**. Quien lea el libro verá las dos y sabrá que
    /// hubo un error y cuándo se corrigió, que es justo lo que un borrado
    /// destruiría.
    pub fn reverse(&mut self, id: &str, reversal_id: impl Into<String>) -> Result<(), LedgerError> {
        let Some(index) = self.applied.get(id) else {
            return Err(LedgerError::UnknownReversal { id: id.to_string() });
        };
        let original = self.journal[*index].clone();
        let entries =
            original.entries.iter().map(|entry| Entry::new(entry.account.clone(), entry.amount.negate())).collect();
        let mut reversal = Transaction::new(reversal_id, format!("reversa de {id}: {}", original.description), entries);
        reversal.reverses = Some(id.to_string());
        self.post(reversal)
    }

    /// Saldo de una cuenta en una moneda. Una cuenta sin movimientos vale cero,
    /// no «no existe»: preguntar por ella es legítimo.
    pub fn balance(&self, account: &str, currency: Currency) -> Money {
        self.balances.get(&(account.to_string(), currency)).copied().unwrap_or_else(|| Money::zero(currency))
    }

    /// Todas las transacciones, en orden de aplicación.
    pub fn journal(&self) -> &[Transaction] {
        &self.journal
    }

    /// La invariante que hace que el libro signifique algo: la suma de **todos**
    /// los saldos de cada moneda es cero.
    ///
    /// Si no lo fuera, habría dinero que entró en una cuenta sin salir de otra.
    pub fn is_balanced(&self) -> bool {
        let mut totals: BTreeMap<Currency, i128> = BTreeMap::new();
        for ((_, currency), amount) in &self.balances {
            *totals.entry(*currency).or_default() += amount.minor_units();
        }
        totals.values().all(|total| *total == 0)
    }

    /// Reconstruye los saldos desde el diario. Es lo que permite comprobar que
    /// la caché no se ha desviado de la historia — y, en un incidente, volver a
    /// levantar el estado desde los hechos.
    pub fn replay(&self) -> BTreeMap<(AccountId, Currency), Money> {
        let mut balances: BTreeMap<(AccountId, Currency), Money> = BTreeMap::new();
        for transaction in &self.journal {
            for entry in &transaction.entries {
                let key = (entry.account.clone(), entry.amount.currency());
                let current = balances.get(&key).copied().unwrap_or_else(|| Money::zero(entry.amount.currency()));
                balances.insert(key, current.checked_add(entry.amount).expect("el diario ya se validó al aplicarse"));
            }
        }
        balances
    }

    fn check_balanced(&self, entries: &[Entry]) -> Result<(), LedgerError> {
        let mut totals: BTreeMap<Currency, Money> = BTreeMap::new();
        for entry in entries {
            let currency = entry.amount.currency();
            let running = totals.get(&currency).copied().unwrap_or_else(|| Money::zero(currency));
            totals.insert(currency, running.checked_add(entry.amount)?);
        }
        for (currency, total) in totals {
            if !total.is_zero() {
                return Err(LedgerError::Unbalanced { currency, difference: total });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clp(units: i128) -> Money {
        Money::new(units, Currency::Clp)
    }

    fn transfer(id: &str, amount: i128) -> Transaction {
        Transaction::new(
            id,
            "traspaso",
            vec![Entry::new("cliente:ana:efectivo", clp(-amount)), Entry::new("cliente:beto:efectivo", clp(amount))],
        )
    }

    #[test]
    fn a_transaction_that_does_not_balance_is_rejected() {
        // La regla central. Sin ella el libro cuadra cuando le conviene.
        let mut ledger = Ledger::new();
        let bad = Transaction::new("t1", "dinero de la nada", vec![Entry::new("cliente:ana:efectivo", clp(1_000))]);
        match ledger.post(bad) {
            Err(LedgerError::Unbalanced { currency, difference }) => {
                assert_eq!(currency, Currency::Clp);
                assert_eq!(difference, clp(1_000));
            }
            other => panic!("se esperaba un descuadre y llegó {other:?}"),
        }
        assert!(ledger.journal().is_empty(), "una transacción rechazada no entra en el diario");
    }

    #[test]
    fn debits_equal_credits_after_any_sequence() {
        let mut ledger = Ledger::new();
        for (index, amount) in [1_000, 250, 7, 999_999].into_iter().enumerate() {
            ledger.post(transfer(&format!("t{index}"), amount)).expect("cuadra");
        }
        assert!(ledger.is_balanced(), "la suma de todos los saldos de una moneda tiene que ser cero");
        assert_eq!(ledger.balance("cliente:beto:efectivo", Currency::Clp), clp(1_001_256));
        assert_eq!(ledger.balance("cliente:ana:efectivo", Currency::Clp), clp(-1_001_256));
    }

    #[test]
    fn the_same_transaction_is_never_applied_twice() {
        // El fallo más caro de un sistema con reintentos: cobrar dos veces.
        let mut ledger = Ledger::new();
        ledger.post(transfer("pago-001", 5_000)).expect("primera");
        assert_eq!(ledger.post(transfer("pago-001", 5_000)), Err(LedgerError::Duplicate { id: "pago-001".into() }));
        assert_eq!(ledger.balance("cliente:beto:efectivo", Currency::Clp), clp(5_000), "el saldo no se dobla");
        assert_eq!(ledger.journal().len(), 1);
    }

    #[test]
    fn a_reversal_does_not_erase_the_original() {
        let mut ledger = Ledger::new();
        ledger.post(transfer("pago-002", 3_000)).expect("original");
        ledger.reverse("pago-002", "rev-pago-002").expect("reversa");

        assert_eq!(ledger.balance("cliente:beto:efectivo", Currency::Clp), clp(0), "el efecto se deshace");
        assert_eq!(ledger.journal().len(), 2, "pero los dos hechos quedan");
        assert_eq!(ledger.journal()[0].id, "pago-002");
        assert_eq!(ledger.journal()[1].reverses.as_deref(), Some("pago-002"));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn nothing_is_reversed_twice() {
        let mut ledger = Ledger::new();
        ledger.post(transfer("pago-003", 100)).expect("original");
        ledger.reverse("pago-003", "rev-a").expect("primera reversa");
        assert_eq!(
            ledger.reverse("pago-003", "rev-b"),
            Err(LedgerError::AlreadyReversed { id: "pago-003".into() }),
            "revertir dos veces devolvería el dinero dos veces"
        );
    }

    #[test]
    fn nothing_that_is_not_there_can_be_reversed() {
        let mut ledger = Ledger::new();
        assert_eq!(ledger.reverse("no-existe", "rev"), Err(LedgerError::UnknownReversal { id: "no-existe".into() }));
    }

    #[test]
    fn the_balances_can_be_rebuilt_from_the_journal() {
        // Los saldos son caché. Si se desviaran de la historia, la historia
        // manda — y esto es lo que permite comprobarlo y, en un incidente,
        // levantar el estado desde los hechos.
        let mut ledger = Ledger::new();
        ledger.post(transfer("a", 10)).expect("a");
        ledger.post(transfer("b", 20)).expect("b");
        ledger.reverse("a", "rev-a").expect("reversa");

        let rebuilt = ledger.replay();
        assert_eq!(rebuilt.get(&("cliente:beto:efectivo".to_string(), Currency::Clp)).copied(), Some(clp(20)));
        assert_eq!(rebuilt, ledger.balances);
    }

    #[test]
    fn a_transaction_can_move_two_currencies_if_each_one_balances() {
        // Una compra de divisa: sale peso, entra dólar. Cada moneda cuadra por
        // separado; exigir que cuadren «juntas» no significaría nada.
        let mut ledger = Ledger::new();
        let fx = Transaction::new(
            "fx-1",
            "compra de dólares",
            vec![
                Entry::new("cliente:ana:efectivo", clp(-950_000)),
                Entry::new("mesa:caja", clp(950_000)),
                Entry::new("cliente:ana:usd", Money::new(100_000, Currency::Usd)),
                Entry::new("mesa:usd", Money::new(-100_000, Currency::Usd)),
            ],
        );
        ledger.post(fx).expect("cada moneda cuadra");
        assert!(ledger.is_balanced());
        assert_eq!(ledger.balance("cliente:ana:usd", Currency::Usd), Money::new(100_000, Currency::Usd));
    }

    #[test]
    fn an_empty_transaction_is_rejected() {
        let mut ledger = Ledger::new();
        assert_eq!(ledger.post(Transaction::new("vacia", "nada", vec![])), Err(LedgerError::Empty));
    }
}
