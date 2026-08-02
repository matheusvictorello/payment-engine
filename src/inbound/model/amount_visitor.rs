use crate::core::model::Amount;
use serde::de;
use std::fmt;

pub struct AmountVisitor;

impl<'de> de::Visitor<'de> for AmountVisitor {
    type Value = Amount;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a decimal number with up to 4 decimal places")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Amount, E> {
        let v = v.trim();

        let (int_str, dec_str) = v.split_once('.').unwrap_or((v, ""));

        if dec_str.len() > 4 {
            return Err(E::custom(format!("too many decimal places in '{v}'")));
        }

        let int_part = int_str.parse::<u64>().map_err(E::custom)?;

        // Left-justify in a 4-char field, padding with zeros on the right.
        let dec_str = format!("{dec_str:0<4}").parse::<u64>().map_err(E::custom)?;

        Ok(Amount(int_part * 10_000 + dec_str))
    }
}
