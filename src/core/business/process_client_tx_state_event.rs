use crate::core::business::Store;
use crate::core::model::{Account, ClientID, ClientTxStateEvent};
use futures::{Stream, StreamExt};

fn process_client_tx_state_event<T>(
    store: &mut T,
    event: ClientTxStateEvent,
) -> Result<(), ClientTxStateEvent>
where
    T: Store<ClientID, Account>,
{
    let ClientTxStateEvent {
        client_id,
        tx_id,
        tx_state_event,
    } = event;

    let key = client_id;

    let account = store.remove(&key).unwrap_or(Account::new());

    match account.apply(tx_state_event) {
        Ok(new_account) => {
            store.insert(key, new_account);

            Ok(())
        }
        Err((unchanged_account, unprocessed_event)) => {
            store.insert(key, unchanged_account);

            Err(ClientTxStateEvent {
                client_id,
                tx_id,
                tx_state_event: unprocessed_event,
            })
        }
    }
}

pub async fn process_client_tx_state_event_stream<T, S>(store: &mut T, mut stream: S)
where
    T: Store<ClientID, Account>,
    S: Stream<Item = ClientTxStateEvent> + Unpin,
{
    while let Some(event) = stream.next().await {
        if let Err(err) = process_client_tx_state_event(store, event) {
            eprintln!("Ignored tx state event: {:?}", err);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::model::{Amount, TxStateEvent};
    use futures::stream;
    use std::collections::HashMap;

    type TestStore = HashMap<ClientID, Account>;

    #[test]
    fn test_process_client_tx_state_event_deposit_on_new_account() {
        let mut store: TestStore = HashMap::new();

        let result = process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(10_000),
                },
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount(10_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_process_client_tx_state_event_withdrawal_success() {
        let mut store: TestStore = HashMap::new();
        store.insert(
            1,
            Account {
                available: Amount(10_000),
                held: Amount::zero(),
                locked: false,
            },
        );

        let result = process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 2,
                tx_state_event: TxStateEvent::Withdrawal {
                    amount: Amount(4_000),
                },
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount(6_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_process_client_tx_state_event_withdrawal_insufficient_funds_errors() {
        let mut store: TestStore = HashMap::new();
        store.insert(
            1,
            Account {
                available: Amount(1_000),
                held: Amount::zero(),
                locked: false,
            },
        );

        let result = process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 2,
                tx_state_event: TxStateEvent::Withdrawal {
                    amount: Amount(1_001),
                },
            },
        );

        assert_eq!(
            result,
            Err(ClientTxStateEvent {
                client_id: 1,
                tx_id: 2,
                tx_state_event: TxStateEvent::Withdrawal {
                    amount: Amount(1_001)
                },
            }),
        );
        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount(1_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_process_client_tx_state_event_dispute_then_resolve() {
        let mut store: TestStore = HashMap::new();

        process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(10_000),
                },
            },
        )
        .unwrap();
        process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Dispute {
                    amount: Amount(10_000),
                },
            },
        )
        .unwrap();

        let result = process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Resolve {
                    amount: Amount(10_000),
                },
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount(10_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[test]
    fn test_process_client_tx_state_event_dispute_then_chargeback_locks_account() {
        let mut store: TestStore = HashMap::new();

        process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(10_000),
                },
            },
        )
        .unwrap();
        process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Dispute {
                    amount: Amount(10_000),
                },
            },
        )
        .unwrap();

        let result = process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Chargeback {
                    amount: Amount(10_000),
                },
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount::zero(),
                held: Amount::zero(),
                locked: true
            }),
        );
    }

    #[test]
    fn test_process_client_tx_state_event_locked_account_rejects_further_events() {
        let mut store: TestStore = HashMap::new();
        store.insert(
            1,
            Account {
                available: Amount(5_000),
                held: Amount::zero(),
                locked: true,
            },
        );

        let result = process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 5,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(1_000),
                },
            },
        );

        assert_eq!(
            result,
            Err(ClientTxStateEvent {
                client_id: 1,
                tx_id: 5,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(1_000)
                },
            }),
        );
        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount(5_000),
                held: Amount::zero(),
                locked: true
            }),
        );
    }

    #[test]
    fn test_process_client_tx_state_event_tracks_independent_clients() {
        let mut store: TestStore = HashMap::new();

        process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(1_000),
                },
            },
        )
        .unwrap();
        process_client_tx_state_event(
            &mut store,
            ClientTxStateEvent {
                client_id: 2,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(2_000),
                },
            },
        )
        .unwrap();

        assert_eq!(store.len(), 2);
        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount(1_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
        assert_eq!(
            store.get(&2),
            Some(&Account {
                available: Amount(2_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[tokio::test]
    async fn test_process_client_tx_state_event_stream_applies_valid_events() {
        let mut store: TestStore = HashMap::new();

        let events = vec![
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(10_000),
                },
            },
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 2,
                tx_state_event: TxStateEvent::Withdrawal {
                    amount: Amount(4_000),
                },
            },
        ];

        process_client_tx_state_event_stream(&mut store, stream::iter(events)).await;

        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount(6_000),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[tokio::test]
    async fn test_process_client_tx_state_event_stream_drops_invalid_events() {
        let mut store: TestStore = HashMap::new();

        let events = vec![
            // invalid: withdrawing more than the available balance
            ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Withdrawal {
                    amount: Amount(1_000),
                },
            },
            ClientTxStateEvent {
                client_id: 2,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(500),
                },
            },
        ];

        process_client_tx_state_event_stream(&mut store, stream::iter(events)).await;

        assert_eq!(
            store.get(&1),
            Some(&Account {
                available: Amount::zero(),
                held: Amount::zero(),
                locked: false
            }),
        );
        assert_eq!(
            store.get(&2),
            Some(&Account {
                available: Amount(500),
                held: Amount::zero(),
                locked: false,
            }),
        );
    }

    #[tokio::test]
    async fn test_process_client_tx_state_event_stream_empty_stream() {
        let mut store: TestStore = HashMap::new();

        process_client_tx_state_event_stream(
            &mut store,
            stream::iter(Vec::<ClientTxStateEvent>::new()),
        )
        .await;

        assert!(store.is_empty());
    }
}
