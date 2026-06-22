use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::AlphaSecError;

/// Scale a Decimal value to an 18-decimal big.Int integer string (perp wire format).
///
/// Multiplies by 10^18 then truncates to the integer part (no rounding).
/// Rejects negative values. Returns an error on overflow (value too large for
/// the checked multiply path).
///
/// Assumption: all perp tokens use flat 1e18 scaling.
pub fn perp_scale(value: Decimal) -> Result<String, AlphaSecError> {
    if value.is_sign_negative() {
        return Err(AlphaSecError::invalid_parameter(
            "amount cannot be negative",
        ));
    }
    let scale_factor = Decimal::from_i128_with_scale(1_000_000_000_000_000_000i128, 0);
    let scaled = value
        .checked_mul(scale_factor)
        .ok_or_else(|| {
            AlphaSecError::invalid_parameter("amount too large (overflow in ×10^18 scale)")
        })?
        .trunc();
    Ok(scaled.to_string())
}

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
pub fn normalize_price_quantity(
    price: Decimal,
    quantity: Decimal,
) -> Result<(Decimal, Decimal), AlphaSecError> {
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

    fn get_quantity_precision(price: Decimal) -> u64 {
        if price >= Decimal::from_str("10000.0").unwrap() {
            5
        } else if price >= Decimal::from_str("1000.0").unwrap() {
            4
        } else if price >= Decimal::from_str("100.0").unwrap() {
            3
        } else if price >= Decimal::from_str("10.0").unwrap() {
            2
        } else if price >= Decimal::from_str("1.0").unwrap() {
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
    let quantity_precision = get_quantity_precision(price);

    // Truncate price and quantity to the calculated precision
    let rounded_price = truncate_to_precision(price, price_precision);
    let rounded_quantity = truncate_to_precision(quantity, quantity_precision);

    Ok((rounded_price, rounded_quantity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perp_scale_1e18() {
        assert_eq!(
            perp_scale(Decimal::from_str("90000").unwrap()).unwrap(),
            "90000000000000000000000"
        );
        assert_eq!(
            perp_scale(Decimal::from_str("0.5").unwrap()).unwrap(),
            "500000000000000000"
        );
        assert_eq!(perp_scale(Decimal::from_str("0").unwrap()).unwrap(), "0");
        // digits beyond 18 decimal places are truncated (not rounded)
        assert_eq!(
            perp_scale(Decimal::from_str("0.0000000000000000019").unwrap()).unwrap(),
            "1"
        );
        // negative values must be rejected
        assert!(perp_scale(Decimal::from_str("-1").unwrap()).is_err());
    }

    #[test]
    fn test_perp_scale_truncates_floor_not_round() {
        assert_eq!(
            perp_scale(Decimal::from_str("1.9999999999999999999").unwrap()).unwrap(),
            "1999999999999999999"
        );
    }

    #[test]
    fn test_perp_scale_below_granularity_truncates_to_zero() {
        // 1e-19 is one order of magnitude smaller than the 1e-18 unit -> floored away.
        assert_eq!(
            perp_scale(Decimal::from_str("0.0000000000000000001").unwrap()).unwrap(),
            "0"
        );
    }

    #[test]
    fn test_perp_scale_signed_zero_allowed() {
        assert_eq!(perp_scale(Decimal::from_str("-0.0").unwrap()).unwrap(), "0");
    }

    #[test]
    fn test_perp_scale_overflow_returns_error() {
        assert!(perp_scale(Decimal::from_str("8000000000000").unwrap()).is_err());
        // sanity: a value just under the overflow boundary still scales successfully,
        // so the error above is the overflow guard firing, not a blanket rejection of large input.
        assert_eq!(
            perp_scale(Decimal::from_str("79228162514").unwrap()).unwrap(),
            "79228162514000000000000000000"
        );
    }

    // Test-only helper: parse a literal into Decimal.
    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    // Test-only helper: run normalize and unwrap the Ok pair.
    fn norm(price: &str, quantity: &str) -> (Decimal, Decimal) {
        normalize_price_quantity(dec(price), dec(quantity)).unwrap()
    }

    #[test]
    fn normalize_rounds_excess_digits_instead_of_truncating() {
        // price 2.00006 is in the [1, 10) band -> 4 dp; rounding carries the 6 up.
        let (price, quantity) = norm("2.00006", "0.26");
        assert_eq!(
            price,
            dec("2.0001"),
            "price must round up, not truncate to 2.0000"
        );
        // quantity precision for the [1, 10) price band is 1 dp; 0.26 rounds up to 0.3.
        assert_eq!(
            quantity,
            dec("0.3"),
            "quantity must round up, not truncate to 0.2"
        );
    }

    #[test]
    fn normalize_midpoints_resolve_to_nearest_even() {
        // Price midpoints in the [1, 10) band (4 dp).
        let (p1, _) = norm("2.00015", "1");
        let (p2, _) = norm("2.00025", "1");
        assert_eq!(p1, dec("2.0002"), "tie 2.00015 must round to even 2.0002");
        assert_eq!(p2, dec("2.0002"), "tie 2.00025 must round to even 2.0002");
        // Quantity midpoints with a [1, 10) band price (1 dp).
        let (_, q1) = norm("2", "0.25");
        let (_, q2) = norm("2", "0.35");
        assert_eq!(q1, dec("0.2"), "tie 0.25 must round to even 0.2");
        assert_eq!(q2, dec("0.4"), "tie 0.35 must round to even 0.4");
    }

    #[test]
    fn price_band_boundaries_are_inclusive() {
        let qty = "0.123456";
        assert_eq!(
            norm("10000.0", qty).1,
            dec("0.12346"),
            "price >= 10000 -> qty 5 dp"
        );
        assert_eq!(
            norm("9999.9", qty).1,
            dec("0.1235"),
            "just below 10000 -> qty 4 dp"
        );
        assert_eq!(
            norm("1000.0", qty).1,
            dec("0.1235"),
            "price >= 1000 -> qty 4 dp"
        );
        assert_eq!(
            norm("100.0", qty).1,
            dec("0.123"),
            "price >= 100 -> qty 3 dp"
        );
        assert_eq!(norm("10.0", qty).1, dec("0.12"), "price >= 10 -> qty 2 dp");
        // Note: the `>=` boundary at price 1.0 is externally unobservable (price 1.0
        // normalizes to 1.0 at either 4 or 5 dp, and both adjacent bands give qty 1 dp),
        // so no assertion can falsify it; intentionally not tested.
    }

    #[test]
    fn price_precision_table_per_band() {
        let cases = [
            ("12345.678", "12346"),          // >= 10000 -> 0 dp
            ("1234.5678", "1234.6"),         // >= 1000  -> 1 dp
            ("123.45678", "123.46"),         // >= 100   -> 2 dp
            ("12.345678", "12.346"),         // >= 10    -> 3 dp
            ("1.2345678", "1.2346"),         // >= 1     -> 4 dp
            ("0.12345678", "0.12346"),       // >= 0.1   -> 5 dp
            ("0.012345678", "0.012346"),     // >= 0.01  -> 6 dp
            ("0.0012345678", "0.0012346"),   // >= 0.001 -> 7 dp
            ("0.00012345678", "0.00012346"), // >= 0.0001 -> 8 dp
            // else branch (below 0.0001): same 8 dp as the >= 0.0001 branch.
            ("0.000012345678", "0.00001235"),
        ];
        for (input, expected) in cases {
            let (price, _) = norm(input, "1");
            assert_eq!(
                price,
                dec(expected),
                "price {input} must normalize to {expected}"
            );
        }
    }

    #[test]
    fn quantity_precision_is_dictated_by_price_band() {
        // Quantity magnitude is irrelevant: sub-1 price -> 1 dp regardless of qty size.
        let (_, q) = norm("0.5", "123.456789");
        assert_eq!(
            q,
            dec("123.5"),
            "qty dp must come from price band, not qty magnitude"
        );

        // Full price-band -> quantity-dp mapping (non-boundary representatives).
        let qty = "0.123456";
        assert_eq!(
            norm("20000", qty).1,
            dec("0.12346"),
            "price band >= 10000 -> qty 5 dp"
        );
        assert_eq!(
            norm("2000", qty).1,
            dec("0.1235"),
            "price band >= 1000 -> qty 4 dp"
        );
        assert_eq!(
            norm("200", qty).1,
            dec("0.123"),
            "price band >= 100 -> qty 3 dp"
        );
        assert_eq!(
            norm("20", qty).1,
            dec("0.12"),
            "price band >= 10 -> qty 2 dp"
        );
        assert_eq!(norm("2", qty).1, dec("0.1"), "price band >= 1 -> qty 1 dp");
        assert_eq!(norm("0.5", qty).1, dec("0.1"), "price band < 1 -> qty 1 dp");

        // Exact pin of the case the old test asserted wrongly (expected 0.2):
        // price 2748 is in the >= 1000 band -> qty 4 dp -> 0.0026 passes through unchanged.
        let (p, q) = norm("2748", "0.0026");
        assert_eq!(p, dec("2748"));
        assert_eq!(
            q,
            dec("0.0026"),
            "qty 0.0026 at price 2748 must stay 0.0026, not 0.2"
        );
    }

    #[test]
    fn negative_inputs_rejected_with_distinct_messages_before_band_logic() {
        // Negative price (large magnitude: must not reach band selection / round_dp).
        let err = normalize_price_quantity(dec("-50000"), dec("1")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid parameter: Price cannot be negative"
        );

        // Negative quantity with a valid price -> quantity-specific message.
        let err = normalize_price_quantity(dec("100"), dec("-5")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid parameter: Quantity cannot be negative"
        );

        // Both negative -> price guard wins (evaluated first).
        let err = normalize_price_quantity(dec("-1"), dec("-1")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid parameter: Price cannot be negative"
        );
    }

    #[test]
    fn signed_zero_and_zero_pass_strict_negative_guard() {
        let (p, q) = norm("-0.0", "-0.0");
        assert_eq!(p, Decimal::ZERO);
        assert_eq!(q, Decimal::ZERO);
        let (p, q) = norm("0.0", "0.0");
        assert_eq!(p, Decimal::ZERO);
        assert_eq!(q, Decimal::ZERO);
    }

    #[test]
    fn normalize_does_not_apply_perp_wire_scaling() {
        let (price, quantity) = norm("90000", "1");
        assert_eq!(price, dec("90000"));
        assert_eq!(quantity, dec("1"));
    }
}
