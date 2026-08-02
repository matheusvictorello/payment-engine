use crate::core::model::{Amount, ClientID, ClientTxEvent, TxEvent, TxID};
use crate::inbound::model::CsvTxTy;
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Debug)]
pub struct CsvClientTxRecord {
    #[serde(rename = "type")]
    pub ty: CsvTxTy,
    pub client: ClientID,
    pub tx: TxID,
    pub amount: Option<Amount>,
}

impl TryFrom<CsvClientTxRecord> for ClientTxEvent {
    type Error = CsvClientTxRecord;

    fn try_from(value: CsvClientTxRecord) -> Result<Self, Self::Error> {
        match value {
            CsvClientTxRecord {
                ty: CsvTxTy::Deposit,
                client,
                tx,
                amount: Some(amount),
            } => Ok(Self {
                client_id: client,
                tx_id: tx,
                tx_event: TxEvent::Deposit { amount },
            }),
            CsvClientTxRecord {
                ty: CsvTxTy::Withdrawal,
                client,
                tx,
                amount: Some(amount),
            } => Ok(Self {
                client_id: client,
                tx_id: tx,
                tx_event: TxEvent::Withdrawal { amount },
            }),
            CsvClientTxRecord {
                ty: CsvTxTy::Dispute,
                client,
                tx,
                amount: None,
            } => Ok(Self {
                client_id: client,
                tx_id: tx,
                tx_event: TxEvent::Dispute,
            }),
            CsvClientTxRecord {
                ty: CsvTxTy::Resolve,
                client,
                tx,
                amount: None,
            } => Ok(Self {
                client_id: client,
                tx_id: tx,
                tx_event: TxEvent::Resolve,
            }),
            CsvClientTxRecord {
                ty: CsvTxTy::Chargeback,
                client,
                tx,
                amount: None,
            } => Ok(Self {
                client_id: client,
                tx_id: tx,
                tx_event: TxEvent::Chargeback,
            }),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use csv_async::AsyncReaderBuilder;
    use futures::stream::StreamExt;
    use std::io::Cursor;

    #[test]
    fn test_try_from_deposit() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Deposit,
            client: 1,
            tx: 1,
            amount: Some(Amount(10_000)),
        };

        assert_eq!(
            ClientTxEvent::try_from(record),
            Ok(ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(10_000)
                },
            }),
        );
    }

    #[test]
    fn test_try_from_withdrawal() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Withdrawal,
            client: 1,
            tx: 1,
            amount: Some(Amount(10_000)),
        };

        assert_eq!(
            ClientTxEvent::try_from(record),
            Ok(ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Withdrawal {
                    amount: Amount(10_000)
                },
            }),
        );
    }

    #[test]
    fn test_try_from_dispute() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Dispute,
            client: 1,
            tx: 1,
            amount: None,
        };

        assert_eq!(
            ClientTxEvent::try_from(record),
            Ok(ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Dispute,
            }),
        );
    }

    #[test]
    fn test_try_from_resolve() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Resolve,
            client: 1,
            tx: 1,
            amount: None,
        };

        assert_eq!(
            ClientTxEvent::try_from(record),
            Ok(ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Resolve,
            }),
        );
    }

    #[test]
    fn test_try_from_chargeback() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Chargeback,
            client: 1,
            tx: 1,
            amount: None,
        };

        assert_eq!(
            ClientTxEvent::try_from(record),
            Ok(ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Chargeback,
            }),
        );
    }

    #[test]
    fn test_try_from_deposit_without_amount_errors() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Deposit,
            client: 1,
            tx: 1,
            amount: None,
        };

        let err = ClientTxEvent::try_from(record).unwrap_err();

        assert!(matches!(err.ty, CsvTxTy::Deposit));
        assert_eq!(err.client, 1);
        assert_eq!(err.tx, 1);
        assert_eq!(err.amount, None);
    }

    #[test]
    fn test_try_from_withdrawal_without_amount_errors() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Withdrawal,
            client: 1,
            tx: 1,
            amount: None,
        };

        assert!(ClientTxEvent::try_from(record).is_err());
    }

    #[test]
    fn test_try_from_dispute_with_amount_errors() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Dispute,
            client: 1,
            tx: 1,
            amount: Some(Amount(10_000)),
        };

        let err = ClientTxEvent::try_from(record).unwrap_err();

        assert!(matches!(err.ty, CsvTxTy::Dispute));
        assert_eq!(err.amount, Some(Amount(10_000)));
    }

    #[test]
    fn test_try_from_resolve_with_amount_errors() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Resolve,
            client: 1,
            tx: 1,
            amount: Some(Amount(10_000)),
        };

        assert!(ClientTxEvent::try_from(record).is_err());
    }

    #[test]
    fn test_try_from_chargeback_with_amount_errors() {
        let record = CsvClientTxRecord {
            ty: CsvTxTy::Chargeback,
            client: 1,
            tx: 1,
            amount: Some(Amount(10_000)),
        };

        assert!(ClientTxEvent::try_from(record).is_err());
    }

    async fn deserialize_csv(csv: &str) -> Vec<CsvClientTxRecord> {
        let deserializer = AsyncReaderBuilder::new()
            .delimiter(b',')
            .has_headers(true)
            .trim(csv_async::Trim::All)
            .create_deserializer(Cursor::new(csv.as_bytes().to_vec()));

        deserializer
            .into_deserialize::<CsvClientTxRecord>()
            .map(|result| result.expect("valid csv record"))
            .collect()
            .await
    }

    #[tokio::test]
    async fn test_csv_deserialize_deposit_row() {
        let records = deserialize_csv("type,client,tx,amount\ndeposit,1,1,1.5\n").await;

        assert_eq!(records.len(), 1);
        assert_eq!(
            ClientTxEvent::try_from(records.into_iter().next().unwrap()).unwrap(),
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(15_000)
                },
            },
        );
    }

    #[tokio::test]
    async fn test_csv_deserialize_dispute_row_with_blank_amount() {
        let records = deserialize_csv("type,client,tx,amount\ndispute,1,1,\n").await;

        assert_eq!(records.len(), 1);
        assert_eq!(
            ClientTxEvent::try_from(records.into_iter().next().unwrap()).unwrap(),
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Dispute,
            },
        );
    }

    #[tokio::test]
    async fn test_csv_deserialize_row_missing_amount_column() {
        let records = deserialize_csv("type,client,tx\nresolve,1,1\n").await;

        assert_eq!(records.len(), 1);
        assert_eq!(
            ClientTxEvent::try_from(records.into_iter().next().unwrap()).unwrap(),
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Resolve,
            },
        );
    }
}
