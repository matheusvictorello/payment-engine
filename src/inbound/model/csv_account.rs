use crate::core::model::{Account, Amount, ClientID};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug)]
pub struct CsvAccount {
    client: ClientID,
    available: Amount,
    held: Amount,
    total: Amount,
    locked: bool,
}

impl From<(ClientID, Account)> for CsvAccount {
    fn from((client, account): (ClientID, Account)) -> Self {
        let Account {
            available,
            held,
            locked,
        } = account;

        Self {
            client,
            available,
            held,
            total: available + held,
            locked,
        }
    }
}
