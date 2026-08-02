use std::ops::{Add, AddAssign, SubAssign};

#[derive(PartialOrd, PartialEq, Clone, Copy, Debug)]
pub struct Amount(pub u64);

impl Amount {
    pub fn zero() -> Self {
        Self(0)
    }
}

impl Add for Amount {
    type Output = Self;

    fn add(self, rhs: Amount) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Amount) {
        self.0 += rhs.0
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Amount) {
        self.0 -= rhs.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_zero() {
        assert_eq!(Amount::zero(), Amount(0));
    }

    #[test]
    fn test_add() {
        assert_eq!(Amount(1_000) + Amount(500), Amount(1_500));
    }

    #[test]
    fn test_add_zero_is_identity() {
        assert_eq!(Amount(1_000) + Amount::zero(), Amount(1_000));
    }

    #[test]
    fn test_add_assign() {
        let mut amount = Amount(1_000);
        amount += Amount(500);

        assert_eq!(amount, Amount(1_500));
    }

    #[test]
    fn test_sub_assign() {
        let mut amount = Amount(1_000);
        amount -= Amount(500);

        assert_eq!(amount, Amount(500));
    }

    #[test]
    fn test_eq() {
        assert_eq!(Amount(1_000), Amount(1_000));
        assert_ne!(Amount(1_000), Amount(999));
    }

    #[test]
    fn test_ord() {
        assert!(Amount(1_000) > Amount(999));
        assert!(Amount(999) < Amount(1_000));
        assert!(Amount(1_000) >= Amount(1_000));
        assert!(Amount(1_000) <= Amount(1_000));
    }

    #[test]
    fn test_clone_copy() {
        let amount = Amount(1_000);
        let copied = amount;
        let cloned = amount.clone();

        assert_eq!(amount, copied);
        assert_eq!(amount, cloned);
    }

    #[test]
    #[should_panic]
    fn test_sub_assign_overflow_panics() {
        let mut amount = Amount(0);
        amount -= Amount(1);
    }
}
