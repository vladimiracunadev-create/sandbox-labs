//! Dinero exacto. Nunca coma flotante.
//!
//! # Por qué esto es lo primero del dominio
//!
//! `0.1 + 0.2` no es `0.3` en binario, y un céntimo perdido por redondeo en una
//! cuenta de clientes no es un detalle: es un descuadre que hay que explicar a
//! un regulador. Los importes se guardan como **enteros en la unidad mínima**
//! —céntimos para el peso o el euro, y la unidad entera para el yen— junto a su
//! moneda.
//!
//! # Por qué la moneda va pegada al importe
//!
//! Sumar 100 CLP y 100 USD tiene que ser imposible, no incorrecto. Aquí no
//! compila… bueno, aquí *falla en tiempo de ejecución con un error explícito*,
//! que es lo más cerca que se puede estar sin un tipo por moneda. Un ledger que
//! suma monedas distintas cuadra perfectamente y no significa nada.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Un importe en la unidad mínima de su moneda.
///
/// `i128` y no `i64` porque una posición agregada en una moneda sin decimales
/// —yen, peso chileno— y con muchos ceros se acerca al límite de 64 bits antes
/// de lo que parece. El coste es nulo aquí y el desbordamiento silencioso sería
/// caro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    minor_units: i128,
    currency: Currency,
}

/// Las monedas que el simulador entiende, con sus decimales.
///
/// Una lista cerrada y no una cadena libre: `"CLP"` y `"clp"` serían dos monedas
/// distintas para un mapa, y ese es el tipo de error que solo aparece cuando ya
/// hay saldos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    /// Peso chileno. **Cero decimales**: su unidad mínima es el peso.
    Clp,
    Usd,
    Eur,
}

impl Currency {
    /// Cuántos decimales tiene esta moneda al escribirla.
    ///
    /// El peso chileno tiene cero, y ese es justo el caso que rompe el código
    /// que asume «dos decimales siempre».
    pub const fn decimals(self) -> u32 {
        match self {
            Self::Clp => 0,
            Self::Usd | Self::Eur => 2,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Clp => "CLP",
            Self::Usd => "USD",
            Self::Eur => "EUR",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code.trim().to_ascii_uppercase().as_str() {
            "CLP" => Some(Self::Clp),
            "USD" => Some(Self::Usd),
            "EUR" => Some(Self::Eur),
            _ => None,
        }
    }
}

/// Lo que puede salir mal al operar con dinero. Todo explícito: en un ledger no
/// existe el «bueno, algo saldrá».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoneyError {
    /// Se intentó operar con dos monedas distintas.
    CurrencyMismatch { left: Currency, right: Currency },
    /// El resultado no cabe. Preferible a envolver en silencio.
    Overflow,
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrencyMismatch { left, right } => {
                write!(f, "no se pueden mezclar {} y {}: son monedas distintas", left.code(), right.code())
            }
            Self::Overflow => write!(f, "el importe desborda el rango representable"),
        }
    }
}

impl std::error::Error for MoneyError {}

impl Money {
    pub const fn new(minor_units: i128, currency: Currency) -> Self {
        Self { minor_units, currency }
    }

    pub const fn zero(currency: Currency) -> Self {
        Self::new(0, currency)
    }

    pub const fn minor_units(self) -> i128 {
        self.minor_units
    }

    pub const fn currency(self) -> Currency {
        self.currency
    }

    pub const fn is_zero(self) -> bool {
        self.minor_units == 0
    }

    pub const fn is_negative(self) -> bool {
        self.minor_units < 0
    }

    fn same_currency(self, other: Self) -> Result<(), MoneyError> {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err(MoneyError::CurrencyMismatch { left: self.currency, right: other.currency })
        }
    }

    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        self.same_currency(other)?;
        let total = self.minor_units.checked_add(other.minor_units).ok_or(MoneyError::Overflow)?;
        Ok(Self::new(total, self.currency))
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        self.same_currency(other)?;
        let total = self.minor_units.checked_sub(other.minor_units).ok_or(MoneyError::Overflow)?;
        Ok(Self::new(total, self.currency))
    }

    /// El importe con signo cambiado. Es lo que convierte un asiento en su
    /// contrapartida.
    pub fn negate(self) -> Self {
        Self::new(-self.minor_units, self.currency)
    }
}

impl fmt::Display for Money {
    /// Escribe el importe con los decimales de su moneda y su código.
    ///
    /// El signo se pone delante del todo: `-1.234,50` mal escrito como
    /// `1.-234,50` es la clase de detalle que hace ilegible un extracto.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let decimals = self.currency.decimals();
        let sign = if self.minor_units < 0 { "-" } else { "" };
        let magnitude = self.minor_units.unsigned_abs();
        if decimals == 0 {
            return write!(f, "{sign}{magnitude} {}", self.currency.code());
        }
        let divisor = 10_u128.pow(decimals);
        write!(
            f,
            "{sign}{}.{:0width$} {}",
            magnitude / divisor,
            magnitude % divisor,
            self.currency.code(),
            width = decimals as usize
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_of_different_currencies_never_adds() {
        // Que sea imposible, no que salga mal. Un ledger que suma monedas
        // distintas cuadra perfectamente y no significa nada.
        let pesos = Money::new(100, Currency::Clp);
        let dollars = Money::new(100, Currency::Usd);
        assert_eq!(
            pesos.checked_add(dollars),
            Err(MoneyError::CurrencyMismatch { left: Currency::Clp, right: Currency::Usd })
        );
        assert!(pesos.checked_sub(dollars).is_err());
    }

    #[test]
    fn the_chilean_peso_has_no_decimals() {
        // El caso que rompe todo el código que asume «dos decimales siempre».
        assert_eq!(Currency::Clp.decimals(), 0);
        assert_eq!(Money::new(1500, Currency::Clp).to_string(), "1500 CLP");
        assert_eq!(Money::new(1500, Currency::Usd).to_string(), "15.00 USD");
    }

    #[test]
    fn the_sign_goes_in_front_of_everything() {
        assert_eq!(Money::new(-1500, Currency::Usd).to_string(), "-15.00 USD");
        assert_eq!(Money::new(-5, Currency::Usd).to_string(), "-0.05 USD");
        assert_eq!(Money::new(-1500, Currency::Clp).to_string(), "-1500 CLP");
    }

    #[test]
    fn amounts_below_the_unit_keep_their_zeros() {
        // `0.5 USD` en vez de `0.50 USD` es medio dólar escrito como cinco
        // céntimos para quien lee rápido.
        assert_eq!(Money::new(50, Currency::Usd).to_string(), "0.50 USD");
        assert_eq!(Money::new(5, Currency::Usd).to_string(), "0.05 USD");
    }

    #[test]
    fn overflow_is_an_error_not_a_wrap() {
        let huge = Money::new(i128::MAX, Currency::Usd);
        assert_eq!(huge.checked_add(Money::new(1, Currency::Usd)), Err(MoneyError::Overflow));
    }

    #[test]
    fn adding_and_subtracting_the_same_amount_returns_to_the_start() {
        let start = Money::new(123_456, Currency::Clp);
        let delta = Money::new(7_890, Currency::Clp);
        assert_eq!(start.checked_add(delta).and_then(|value| value.checked_sub(delta)), Ok(start));
    }

    #[test]
    fn negating_twice_is_the_identity() {
        let amount = Money::new(-42, Currency::Eur);
        assert_eq!(amount.negate().negate(), amount);
    }

    #[test]
    fn currency_codes_round_trip_case_insensitively() {
        for code in ["CLP", "clp", " Clp "] {
            assert_eq!(Currency::parse(code), Some(Currency::Clp));
        }
        assert_eq!(Currency::parse("BTC"), None, "una moneda desconocida no se inventa");
    }
}
