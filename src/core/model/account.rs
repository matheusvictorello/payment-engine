use crate::core::model::{Amount, TxStateEvent};

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Account {
    pub available: Amount,
    pub held: Amount,
    pub locked: bool,
}

impl Account {
    pub fn new() -> Self {
        Self {
            available: Amount::zero(),
            held: Amount::zero(),
            locked: false,
        }
    }

    pub fn apply(mut self, tx_state_event: TxStateEvent) -> Result<Self, (Self, TxStateEvent)> {
        if self.locked {
            return Err((self, tx_state_event));
        }

        match tx_state_event {
            TxStateEvent::Deposit { amount } => {
                self.available += amount;
            }
            TxStateEvent::Withdrawal { amount } => {
                if self.available < amount {
                    return Err((self, tx_state_event));
                }

                self.available -= amount;
            }
            TxStateEvent::Dispute { amount } => {
                if self.available < amount {
                    return Err((self, tx_state_event));
                }

                self.available -= amount;
                self.held += amount;
            }
            TxStateEvent::Resolve { amount } => {
                if self.held < amount {
                    return Err((self, tx_state_event));
                }

                self.available += amount;
                self.held -= amount;
            }
            TxStateEvent::Chargeback { amount } => {
                if self.held < amount {
                    return Err((self, tx_state_event));
                }

                self.held -= amount;
                self.locked = true;
            }
        }

        Ok(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deposit() {
        assert_eq!(
            Account::new().apply(TxStateEvent::Deposit {
                amount: Amount(10_000)
            }),
            Ok(Account {
                available: Amount(10_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_withdrawal() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Withdrawal {
                    amount: Amount(9999)
                }),
            Ok(Account {
                available: Amount(1),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_invalid_withdrawal() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Withdrawal {
                    amount: Amount(10_001)
                }),
            Err((
                Account {
                    available: Amount(10_000),
                    held: Amount::zero(),
                    locked: false,
                },
                TxStateEvent::Withdrawal {
                    amount: Amount(10_001)
                },
            )),
        );
    }

    #[test]
    fn test_dispute() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Dispute {
                    amount: Amount(10_000)
                }),
            Ok(Account {
                available: Amount::zero(),
                held: Amount(10_000),
                locked: false,
            },),
        );
    }

    #[test]
    fn test_invalid_dispute() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Dispute {
                    amount: Amount(10_001)
                }),
            Err((
                Account {
                    available: Amount(10_000),
                    held: Amount::zero(),
                    locked: false,
                },
                TxStateEvent::Dispute {
                    amount: Amount(10_001)
                },
            )),
        );
    }

    #[test]
    fn test_resolve() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Dispute {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Resolve {
                    amount: Amount(10_000)
                }),
            Ok(Account {
                available: Amount(10_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_invalid_resolve() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Dispute {
                    amount: Amount(4_000)
                })
                .unwrap()
                .apply(TxStateEvent::Resolve {
                    amount: Amount(4_001)
                }),
            Err((
                Account {
                    available: Amount(6_000),
                    held: Amount(4_000),
                    locked: false,
                },
                TxStateEvent::Resolve {
                    amount: Amount(4_001)
                },
            )),
        );
    }

    #[test]
    fn test_chargeback() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Dispute {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Chargeback {
                    amount: Amount(10_000)
                }),
            Ok(Account {
                available: Amount::zero(),
                held: Amount::zero(),
                locked: true,
            }),
        );
    }

    #[test]
    fn test_invalid_chargeback() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(10_000)
                })
                .unwrap()
                .apply(TxStateEvent::Dispute {
                    amount: Amount(4_000)
                })
                .unwrap()
                .apply(TxStateEvent::Chargeback {
                    amount: Amount(4_001)
                }),
            Err((
                Account {
                    available: Amount(6_000),
                    held: Amount(4_000),
                    locked: false,
                },
                TxStateEvent::Chargeback {
                    amount: Amount(4_001)
                },
            )),
        );
    }

    #[test]
    fn test_locked_account_rejects_deposit() {
        let locked_account = Account {
            available: Amount(1_000),
            held: Amount::zero(),
            locked: true,
        };

        assert_eq!(
            locked_account.apply(TxStateEvent::Deposit {
                amount: Amount(500)
            }),
            Err((
                locked_account,
                TxStateEvent::Deposit {
                    amount: Amount(500)
                }
            )),
        );
    }

    #[test]
    fn test_locked_account_rejects_withdrawal() {
        let locked_account = Account {
            available: Amount(1_000),
            held: Amount::zero(),
            locked: true,
        };

        assert_eq!(
            locked_account.apply(TxStateEvent::Withdrawal {
                amount: Amount(500)
            }),
            Err((
                locked_account,
                TxStateEvent::Withdrawal {
                    amount: Amount(500)
                }
            )),
        );
    }

    #[test]
    fn test_locked_account_rejects_dispute() {
        let locked_account = Account {
            available: Amount(1_000),
            held: Amount::zero(),
            locked: true,
        };

        assert_eq!(
            locked_account.apply(TxStateEvent::Dispute {
                amount: Amount(500)
            }),
            Err((
                locked_account,
                TxStateEvent::Dispute {
                    amount: Amount(500)
                }
            )),
        );
    }

    #[test]
    fn test_locked_account_rejects_resolve() {
        let locked_account = Account {
            available: Amount::zero(),
            held: Amount(1_000),
            locked: true,
        };

        assert_eq!(
            locked_account.apply(TxStateEvent::Resolve {
                amount: Amount(500)
            }),
            Err((
                locked_account,
                TxStateEvent::Resolve {
                    amount: Amount(500)
                }
            )),
        );
    }

    #[test]
    fn test_locked_account_rejects_chargeback() {
        let locked_account = Account {
            available: Amount::zero(),
            held: Amount(1_000),
            locked: true,
        };

        assert_eq!(
            locked_account.apply(TxStateEvent::Chargeback {
                amount: Amount(500)
            }),
            Err((
                locked_account,
                TxStateEvent::Chargeback {
                    amount: Amount(500)
                }
            )),
        );
    }

    #[test]
    fn test_multiple_deposits_accumulate() {
        assert_eq!(
            Account::new()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(1_000)
                })
                .unwrap()
                .apply(TxStateEvent::Deposit {
                    amount: Amount(2_000)
                }),
            Ok(Account {
                available: Amount(3_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_new_account_is_zeroed_and_unlocked() {
        assert_eq!(
            Account::new(),
            Account {
                available: Amount::zero(),
                held: Amount::zero(),
                locked: false,
            },
        );
    }
}
