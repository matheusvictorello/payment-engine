use crate::core::model::{ClientID, TxID, TxStateEvent};

#[derive(PartialEq, Debug)]
pub struct ClientTxStateEvent {
    pub client_id: ClientID,
    pub tx_id: TxID,
    pub tx_state_event: TxStateEvent,
}
