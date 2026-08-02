use crate::core::model::Amount;

#[derive(PartialEq, Debug)]
pub enum TxStateEvent {
    Deposit { amount: Amount },
    Withdrawal { amount: Amount },
    Dispute { amount: Amount },
    Resolve { amount: Amount },
    Chargeback { amount: Amount },
}
