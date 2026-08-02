use crate::core::model::{Amount, TxEvent, TxStateEvent};

#[derive(PartialEq, Debug)]
pub enum TxState {
    Uninitialized,
    Deposit { amount: Amount },
    Withdrawal,
    Dispute { amount: Amount },
    Resolve,
    Chargeback,
}

impl TxState {
    pub fn new() -> Self {
        Self::Uninitialized
    }

    pub fn apply(self, tx_event: TxEvent) -> Result<(Self, TxStateEvent), (Self, TxEvent)> {
        match (self, tx_event) {
            // From Uninitialized
            (Self::Uninitialized, TxEvent::Deposit { amount }) => {
                Ok((Self::Deposit { amount }, TxStateEvent::Deposit { amount }))
            }
            (Self::Uninitialized, TxEvent::Withdrawal { amount }) => {
                Ok((Self::Withdrawal, TxStateEvent::Withdrawal { amount }))
            }

            // Into Dispute
            (Self::Deposit { amount }, TxEvent::Dispute) => {
                Ok((Self::Dispute { amount }, TxStateEvent::Dispute { amount }))
            }

            // From Dispute
            (Self::Dispute { amount }, TxEvent::Resolve) => {
                Ok((Self::Resolve, TxStateEvent::Resolve { amount }))
            }
            (Self::Dispute { amount }, TxEvent::Chargeback) => {
                Ok((Self::Chargeback, TxStateEvent::Chargeback { amount }))
            }

            // Invalid
            (curr, tx_ty) => Err((curr, tx_ty)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_new() {
        assert_eq!(TxState::new(), TxState::Uninitialized);
    }

    #[test]
    fn test_uninitialized_deposit() {
        assert_eq!(
            TxState::new().apply(TxEvent::Deposit {
                amount: Amount(10_000)
            }),
            Ok((
                TxState::Deposit {
                    amount: Amount(10_000)
                },
                TxStateEvent::Deposit {
                    amount: Amount(10_000)
                },
            )),
        );
    }

    #[test]
    fn test_uninitialized_withdrawal() {
        assert_eq!(
            TxState::new().apply(TxEvent::Withdrawal {
                amount: Amount(10_000)
            }),
            Ok((
                TxState::Withdrawal,
                TxStateEvent::Withdrawal {
                    amount: Amount(10_000)
                },
            )),
        );
    }

    #[test]
    fn test_uninitialized_invalid_events() {
        assert_eq!(
            TxState::Uninitialized.apply(TxEvent::Dispute),
            Err((TxState::Uninitialized, TxEvent::Dispute)),
        );
        assert_eq!(
            TxState::Uninitialized.apply(TxEvent::Resolve),
            Err((TxState::Uninitialized, TxEvent::Resolve)),
        );
        assert_eq!(
            TxState::Uninitialized.apply(TxEvent::Chargeback),
            Err((TxState::Uninitialized, TxEvent::Chargeback)),
        );
    }

    #[test]
    fn test_deposit_to_dispute() {
        assert_eq!(
            (TxState::Deposit {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Dispute),
            Ok((
                TxState::Dispute {
                    amount: Amount(10_000)
                },
                TxStateEvent::Dispute {
                    amount: Amount(10_000)
                },
            )),
        );
    }

    #[test]
    fn test_deposit_invalid_events() {
        assert_eq!(
            (TxState::Deposit {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Deposit {
                amount: Amount(500)
            }),
            Err((
                TxState::Deposit {
                    amount: Amount(10_000)
                },
                TxEvent::Deposit {
                    amount: Amount(500)
                },
            )),
        );
        assert_eq!(
            (TxState::Deposit {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Withdrawal {
                amount: Amount(500)
            }),
            Err((
                TxState::Deposit {
                    amount: Amount(10_000)
                },
                TxEvent::Withdrawal {
                    amount: Amount(500)
                },
            )),
        );
        assert_eq!(
            (TxState::Deposit {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Resolve),
            Err((
                TxState::Deposit {
                    amount: Amount(10_000)
                },
                TxEvent::Resolve
            )),
        );
        assert_eq!(
            (TxState::Deposit {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Chargeback),
            Err((
                TxState::Deposit {
                    amount: Amount(10_000)
                },
                TxEvent::Chargeback
            )),
        );
    }

    #[test]
    fn test_withdrawal_invalid_events() {
        assert_eq!(
            TxState::Withdrawal.apply(TxEvent::Deposit {
                amount: Amount(500)
            }),
            Err((
                TxState::Withdrawal,
                TxEvent::Deposit {
                    amount: Amount(500)
                }
            )),
        );
        assert_eq!(
            TxState::Withdrawal.apply(TxEvent::Withdrawal {
                amount: Amount(500)
            }),
            Err((
                TxState::Withdrawal,
                TxEvent::Withdrawal {
                    amount: Amount(500)
                }
            )),
        );
        assert_eq!(
            TxState::Withdrawal.apply(TxEvent::Dispute),
            Err((TxState::Withdrawal, TxEvent::Dispute)),
        );
        assert_eq!(
            TxState::Withdrawal.apply(TxEvent::Resolve),
            Err((TxState::Withdrawal, TxEvent::Resolve)),
        );
        assert_eq!(
            TxState::Withdrawal.apply(TxEvent::Chargeback),
            Err((TxState::Withdrawal, TxEvent::Chargeback)),
        );
    }

    #[test]
    fn test_dispute_to_resolve() {
        assert_eq!(
            (TxState::Dispute {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Resolve),
            Ok((
                TxState::Resolve,
                TxStateEvent::Resolve {
                    amount: Amount(10_000)
                }
            )),
        );
    }

    #[test]
    fn test_dispute_to_chargeback() {
        assert_eq!(
            (TxState::Dispute {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Chargeback),
            Ok((
                TxState::Chargeback,
                TxStateEvent::Chargeback {
                    amount: Amount(10_000)
                }
            )),
        );
    }

    #[test]
    fn test_dispute_invalid_events() {
        assert_eq!(
            (TxState::Dispute {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Deposit {
                amount: Amount(500)
            }),
            Err((
                TxState::Dispute {
                    amount: Amount(10_000)
                },
                TxEvent::Deposit {
                    amount: Amount(500)
                },
            )),
        );
        assert_eq!(
            (TxState::Dispute {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Withdrawal {
                amount: Amount(500)
            }),
            Err((
                TxState::Dispute {
                    amount: Amount(10_000)
                },
                TxEvent::Withdrawal {
                    amount: Amount(500)
                },
            )),
        );
        assert_eq!(
            (TxState::Dispute {
                amount: Amount(10_000)
            })
            .apply(TxEvent::Dispute),
            Err((
                TxState::Dispute {
                    amount: Amount(10_000)
                },
                TxEvent::Dispute
            )),
        );
    }

    #[test]
    fn test_resolve_invalid_events() {
        assert_eq!(
            TxState::Resolve.apply(TxEvent::Deposit {
                amount: Amount(500)
            }),
            Err((
                TxState::Resolve,
                TxEvent::Deposit {
                    amount: Amount(500)
                }
            )),
        );
        assert_eq!(
            TxState::Resolve.apply(TxEvent::Withdrawal {
                amount: Amount(500)
            }),
            Err((
                TxState::Resolve,
                TxEvent::Withdrawal {
                    amount: Amount(500)
                }
            )),
        );
        assert_eq!(
            TxState::Resolve.apply(TxEvent::Dispute),
            Err((TxState::Resolve, TxEvent::Dispute)),
        );
        assert_eq!(
            TxState::Resolve.apply(TxEvent::Resolve),
            Err((TxState::Resolve, TxEvent::Resolve)),
        );
        assert_eq!(
            TxState::Resolve.apply(TxEvent::Chargeback),
            Err((TxState::Resolve, TxEvent::Chargeback)),
        );
    }

    #[test]
    fn test_chargeback_invalid_events() {
        assert_eq!(
            TxState::Chargeback.apply(TxEvent::Deposit {
                amount: Amount(500)
            }),
            Err((
                TxState::Chargeback,
                TxEvent::Deposit {
                    amount: Amount(500)
                }
            )),
        );
        assert_eq!(
            TxState::Chargeback.apply(TxEvent::Withdrawal {
                amount: Amount(500)
            }),
            Err((
                TxState::Chargeback,
                TxEvent::Withdrawal {
                    amount: Amount(500)
                }
            )),
        );
        assert_eq!(
            TxState::Chargeback.apply(TxEvent::Dispute),
            Err((TxState::Chargeback, TxEvent::Dispute)),
        );
        assert_eq!(
            TxState::Chargeback.apply(TxEvent::Resolve),
            Err((TxState::Chargeback, TxEvent::Resolve)),
        );
        assert_eq!(
            TxState::Chargeback.apply(TxEvent::Chargeback),
            Err((TxState::Chargeback, TxEvent::Chargeback)),
        );
    }
}
