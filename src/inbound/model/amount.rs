use crate::core::model::Amount;
use crate::inbound::model::AmountVisitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Amount, D::Error> {
        d.deserialize_str(AmountVisitor)
    }
}

impl Serialize for Amount {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{}.{:04}", self.0 / 10_000, self.0 % 10_000))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::de::IntoDeserializer;
    use serde::de::value::{Error as ValueError, StrDeserializer};

    fn deserialize_str(s: &str) -> Result<Amount, ValueError> {
        let deserializer: StrDeserializer<ValueError> = s.into_deserializer();
        Amount::deserialize(deserializer)
    }

    #[test]
    fn test_deserialize_integer_only() {
        assert_eq!(deserialize_str("42").unwrap(), Amount(420_000));
    }

    #[test]
    fn test_deserialize_with_decimal() {
        assert_eq!(deserialize_str("1.5").unwrap(), Amount(15_000));
    }

    #[test]
    fn test_deserialize_full_four_decimals() {
        assert_eq!(deserialize_str("1.5000").unwrap(), Amount(15_000));
    }

    #[test]
    fn test_deserialize_leading_zeros_in_decimal() {
        assert_eq!(deserialize_str("1.0005").unwrap(), Amount(10_005));
    }

    #[test]
    fn test_deserialize_zero() {
        assert_eq!(deserialize_str("0.0000").unwrap(), Amount::zero());
    }

    #[test]
    fn test_deserialize_trims_whitespace() {
        assert_eq!(deserialize_str(" 1.5 ").unwrap(), Amount(15_000));
    }

    #[test]
    fn test_deserialize_single_decimal_digit() {
        assert_eq!(deserialize_str("1.1").unwrap(), Amount(11_000));
    }

    #[test]
    fn test_deserialize_too_many_decimal_places_errors() {
        assert!(deserialize_str("1.12345").is_err());
    }

    #[test]
    fn test_deserialize_invalid_integer_part_errors() {
        assert!(deserialize_str("abc.5").is_err());
    }

    #[test]
    fn test_deserialize_invalid_decimal_part_errors() {
        assert!(deserialize_str("1.5a").is_err());
    }

    #[test]
    fn test_deserialize_negative_errors() {
        assert!(deserialize_str("-1.5").is_err());
    }

    #[test]
    fn test_deserialize_empty_string_errors() {
        assert!(deserialize_str("").is_err());
    }

    #[test]
    fn test_deserialize_multiple_dots_errors() {
        assert!(deserialize_str("1.2.3").is_err());
    }
}
