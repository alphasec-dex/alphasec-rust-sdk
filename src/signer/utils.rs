use rust_decimal::Decimal;
use std::str::FromStr;

use crate::AlphaSecError;

/// Truncate a value to the specified number of decimal places (no rounding)
fn truncate_to_precision(value: Decimal, precision: u64) -> Decimal {
    value.round_dp(precision as u32)
}

/// Normalize price and quantity values by truncating them to appropriate precision
///
/// The precision is determined based on the magnitude of the values:
/// - Price precision: 0-8 decimal places depending on price range
/// - Quantity precision: 0-5 decimal places depending on quantity range
///
/// # Arguments
/// * `price` - The price value to normalize (must be non-negative)
/// * `quantity` - The quantity value to normalize (must be non-negative)
///
/// # Returns
/// * `Ok((rounded_price, rounded_quantity))` - The normalized values
/// * `Err(AlphaSecError)` - If price or quantity is negative
pub fn normalize_price_quantity(price: Decimal, quantity: Decimal) -> Result<(Decimal, Decimal), AlphaSecError> {
    fn get_price_precision(price: Decimal) -> u64 {
        if price >= Decimal::from_str("10000.0").unwrap() {
            0
        } else if price >= Decimal::from_str("1000.0").unwrap() {
            1
        } else if price >= Decimal::from_str("100.0").unwrap() {
            2
        } else if price >= Decimal::from_str("10.0").unwrap() {
            3
        } else if price >= Decimal::from_str("1.0").unwrap() {
            4
        } else if price >= Decimal::from_str("0.1").unwrap() {
            5
        } else if price >= Decimal::from_str("0.01").unwrap() {
            6
        } else if price >= Decimal::from_str("0.001").unwrap() {
            7
        } else if price >= Decimal::from_str("0.0001").unwrap() {
            8
        } else {
            8
        }
    }

    fn get_quantity_precision(quantity: Decimal) -> u64 {
        if quantity >= Decimal::from_str("10000.0").unwrap() {
            5
        } else if quantity >= Decimal::from_str("1000.0").unwrap() {
            4
        } else if quantity >= Decimal::from_str("100.0").unwrap() {
            3
        } else if quantity >= Decimal::from_str("10.0").unwrap() {
            2
        } else if quantity >= Decimal::from_str("1.0").unwrap() {
            1
        } else {
            1
        }
    }

    if price < Decimal::from_str("0.0").unwrap() {
        return Err(AlphaSecError::invalid_parameter("Price cannot be negative"));
    }
    if quantity < Decimal::from_str("0.0").unwrap() {
        return Err(AlphaSecError::invalid_parameter(
            "Quantity cannot be negative",
        ));
    }

    let price_precision = get_price_precision(price);
    let quantity_precision = get_quantity_precision(quantity);

    // Truncate price and quantity to the calculated precision
    let rounded_price = truncate_to_precision(price, price_precision);
    let rounded_quantity = truncate_to_precision(quantity, quantity_precision);

    Ok((rounded_price, rounded_quantity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_price_quantity() {
        let (price, quantity) = normalize_price_quantity(Decimal::from_str("100.0").unwrap(), Decimal::from_str("1000.0").unwrap()  ).unwrap();
        assert_eq!(price, Decimal::from_str("100.0").unwrap());
        assert_eq!(quantity, Decimal::from_str("1000.0").unwrap());

        let (price, quantity) = normalize_price_quantity(Decimal::from_str("2748").unwrap(), Decimal::from_str("0.0026").unwrap()).unwrap();
        assert_eq!(price, Decimal::from_str("2748").unwrap());
        assert_eq!(quantity, Decimal::from_str("0.0026").unwrap());
    }
}
