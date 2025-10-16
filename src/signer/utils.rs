use crate::AlphaSecError;

/// Truncate a value to the specified number of decimal places (no rounding)
fn truncate_to_precision(value: f64, precision: u64) -> f64 {
    let multiplier = 10_f64.powi(precision as i32);
    (value * multiplier).trunc() / multiplier
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
pub fn normalize_price_quantity(price: f64, quantity: f64) -> Result<(f64, f64), AlphaSecError> {
    fn get_price_precision(price: f64) -> u64 {
        if price >= 10000.0 {
            0
        } else if price >= 1000.0 {
            1
        } else if price >= 100.0 {
            2
        } else if price >= 10.0 {
            3
        } else if price >= 1.0 {
            4
        } else {
            8
        }
    }

    fn get_quantity_precision(quantity: f64) -> u64 {
        if quantity >= 10000.0 {
            5
        } else if quantity >= 1000.0 {
            4
        } else if quantity >= 100.0 {
            3
        } else if quantity >= 10.0 {
            2
        } else if quantity >= 1.0 {
            1
        } else {
            5
        }
    }

    if price < 0.0 {
         return Err(AlphaSecError::invalid_parameter("Price cannot be negative"));
    }
    if quantity < 0.0 {
        return Err(AlphaSecError::invalid_parameter("Quantity cannot be negative"));
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
        let (price, quantity) = normalize_price_quantity(100.0, 1000.0).unwrap();
        assert_eq!(price, 100.0);
        assert_eq!(quantity, 1000.0);

        let (price, quantity) = normalize_price_quantity(112400.055, 0.2).unwrap();
        assert_eq!(price, 112400.0);
        assert_eq!(quantity, 0.2);
    }
}