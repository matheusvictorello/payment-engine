use crate::core::model::Amount;

#[derive(PartialEq, Debug)]
pub enum TxEvent {
    Deposit { amount: Amount },
    Withdrawal { amount: Amount },
    Dispute,
    Resolve,
    Chargeback,
}
