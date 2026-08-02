use crate::core::model::{ClientID, TxEvent, TxID};

#[derive(PartialEq, Debug)]
pub struct ClientTxEvent {
    pub client_id: ClientID,
    pub tx_id: TxID,
    pub tx_event: TxEvent,
}
