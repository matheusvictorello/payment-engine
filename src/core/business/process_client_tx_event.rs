use crate::core::business::Store;
use crate::core::model::{ClientID, ClientTxEvent, ClientTxStateEvent, TxID, TxState};
use futures::{Sink, SinkExt, Stream, StreamExt};

fn process_client_tx_event<T>(
    store: &mut T,
    event: ClientTxEvent,
) -> Result<ClientTxStateEvent, ClientTxEvent>
where
    T: Store<(ClientID, TxID), TxState>,
{
    let ClientTxEvent {
        client_id,
        tx_id,
        tx_event,
    } = event;

    let key = (client_id, tx_id);

    let state = store.remove(&key).unwrap_or(TxState::new());

    match state.apply(tx_event) {
        Ok((new_state, new_event)) => {
            store.insert(key, new_state);

            Ok(ClientTxStateEvent {
                client_id,
                tx_id,
                tx_state_event: new_event,
            })
        }
        Err((unchanged_state, unprocessed_event)) => {
            store.insert(key, unchanged_state);

            Err(ClientTxEvent {
                client_id,
                tx_id,
                tx_event: unprocessed_event,
            })
        }
    }
}

pub async fn process_client_tx_event_stream<T, S, K>(
    store: &mut T,
    mut stream: S,
    mut sink: K,
) -> Result<(), K::Error>
where
    T: Store<(ClientID, TxID), TxState>,
    S: Stream<Item = ClientTxEvent> + Unpin,
    K: Sink<ClientTxStateEvent> + Unpin,
{
    while let Some(event) = stream.next().await {
        match process_client_tx_event(store, event) {
            Ok(new_event) => {
                sink.send(new_event).await?;
            }
            Err(err) => {
                // dead letter queue
                eprintln!("Ignored tx event: {:?}", err);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::model::{Amount, TxEvent, TxStateEvent};
    use futures::channel::mpsc;
    use futures::stream;
    use std::collections::HashMap;

    type TestStore = HashMap<(ClientID, TxID), TxState>;

    #[test]
    fn test_process_client_tx_event_deposit_on_new_tx() {
        let mut store: TestStore = HashMap::new();

        let result = process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(10_000),
                },
            },
        );

        assert_eq!(
            result,
            Ok(ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(10_000)
                },
            }),
        );
        assert_eq!(
            store.get(&(1, 1)),
            Some(&TxState::Deposit {
                amount: Amount(10_000)
            }),
        );
    }

    #[test]
    fn test_process_client_tx_event_withdrawal_on_new_tx() {
        let mut store: TestStore = HashMap::new();

        let result = process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 2,
                tx_id: 2,
                tx_event: TxEvent::Withdrawal {
                    amount: Amount(5_000),
                },
            },
        );

        assert_eq!(
            result,
            Ok(ClientTxStateEvent {
                client_id: 2,
                tx_id: 2,
                tx_state_event: TxStateEvent::Withdrawal {
                    amount: Amount(5_000)
                },
            }),
        );
        assert_eq!(store.get(&(2, 2)), Some(&TxState::Withdrawal));
    }

    #[test]
    fn test_process_client_tx_event_dispute_after_deposit() {
        let mut store: TestStore = HashMap::new();
        store.insert(
            (1, 1),
            TxState::Deposit {
                amount: Amount(10_000),
            },
        );

        let result = process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Dispute,
            },
        );

        assert_eq!(
            result,
            Ok(ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Dispute {
                    amount: Amount(10_000)
                },
            }),
        );
        assert_eq!(
            store.get(&(1, 1)),
            Some(&TxState::Dispute {
                amount: Amount(10_000)
            }),
        );
    }

    #[test]
    fn test_process_client_tx_event_full_chargeback_sequence() {
        let mut store: TestStore = HashMap::new();

        process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(10_000),
                },
            },
        )
        .unwrap();
        process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Dispute,
            },
        )
        .unwrap();

        let result = process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Chargeback,
            },
        );

        assert_eq!(
            result,
            Ok(ClientTxStateEvent {
                client_id: 1,
                tx_id: 1,
                tx_state_event: TxStateEvent::Chargeback {
                    amount: Amount(10_000)
                },
            }),
        );
        assert_eq!(store.get(&(1, 1)), Some(&TxState::Chargeback));
    }

    #[test]
    fn test_process_client_tx_event_invalid_transition_on_new_tx() {
        let mut store: TestStore = HashMap::new();

        let result = process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Dispute,
            },
        );

        assert_eq!(
            result,
            Err(ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Dispute
            }),
        );
        assert_eq!(store.get(&(1, 1)), Some(&TxState::Uninitialized));
    }

    #[test]
    fn test_process_client_tx_event_invalid_transition_leaves_state_unchanged() {
        let mut store: TestStore = HashMap::new();
        store.insert(
            (1, 1),
            TxState::Deposit {
                amount: Amount(10_000),
            },
        );

        let result = process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Resolve,
            },
        );

        assert_eq!(
            result,
            Err(ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Resolve
            }),
        );
        assert_eq!(
            store.get(&(1, 1)),
            Some(&TxState::Deposit {
                amount: Amount(10_000)
            }),
        );
    }

    #[test]
    fn test_process_client_tx_event_tracks_independent_keys() {
        let mut store: TestStore = HashMap::new();

        process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(1_000),
                },
            },
        )
        .unwrap();
        process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 2,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(2_000),
                },
            },
        )
        .unwrap();
        process_client_tx_event(
            &mut store,
            ClientTxEvent {
                client_id: 1,
                tx_id: 2,
                tx_event: TxEvent::Withdrawal {
                    amount: Amount(500),
                },
            },
        )
        .unwrap();

        assert_eq!(store.len(), 3);
        assert_eq!(
            store.get(&(1, 1)),
            Some(&TxState::Deposit {
                amount: Amount(1_000)
            }),
        );
        assert_eq!(
            store.get(&(2, 1)),
            Some(&TxState::Deposit {
                amount: Amount(2_000)
            }),
        );
        assert_eq!(store.get(&(1, 2)), Some(&TxState::Withdrawal));
    }

    #[tokio::test]
    async fn test_process_client_tx_event_stream_forwards_valid_events_in_order() {
        let mut store: TestStore = HashMap::new();

        let events = vec![
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(10_000),
                },
            },
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Dispute,
            },
        ];

        let (sender, receiver) = mpsc::channel(10);

        let result = process_client_tx_event_stream(&mut store, stream::iter(events), sender).await;

        assert!(result.is_ok());

        let received: Vec<ClientTxStateEvent> = receiver.collect().await;

        assert_eq!(
            received,
            vec![
                ClientTxStateEvent {
                    client_id: 1,
                    tx_id: 1,
                    tx_state_event: TxStateEvent::Deposit {
                        amount: Amount(10_000)
                    },
                },
                ClientTxStateEvent {
                    client_id: 1,
                    tx_id: 1,
                    tx_state_event: TxStateEvent::Dispute {
                        amount: Amount(10_000)
                    },
                },
            ],
        );
        assert_eq!(
            store.get(&(1, 1)),
            Some(&TxState::Dispute {
                amount: Amount(10_000)
            }),
        );
    }

    #[tokio::test]
    async fn test_process_client_tx_event_stream_sends_invalid_events_to_dead_letter() {
        let mut store: TestStore = HashMap::new();

        let events = vec![
            // invalid: no prior deposit to dispute
            ClientTxEvent {
                client_id: 1,
                tx_id: 1,
                tx_event: TxEvent::Resolve,
            },
            ClientTxEvent {
                client_id: 2,
                tx_id: 1,
                tx_event: TxEvent::Deposit {
                    amount: Amount(500),
                },
            },
        ];

        let (sender, receiver) = mpsc::channel(10);

        let result = process_client_tx_event_stream(&mut store, stream::iter(events), sender).await;

        assert!(result.is_ok());

        let received: Vec<ClientTxStateEvent> = receiver.collect().await;

        assert_eq!(
            received,
            vec![ClientTxStateEvent {
                client_id: 2,
                tx_id: 1,
                tx_state_event: TxStateEvent::Deposit {
                    amount: Amount(500)
                },
            }],
        );
        assert_eq!(store.get(&(1, 1)), Some(&TxState::Uninitialized));
        assert_eq!(
            store.get(&(2, 1)),
            Some(&TxState::Deposit {
                amount: Amount(500)
            }),
        );
    }

    #[tokio::test]
    async fn test_process_client_tx_event_stream_empty_stream() {
        let mut store: TestStore = HashMap::new();

        let (sender, receiver) = mpsc::channel(10);

        let result = process_client_tx_event_stream(
            &mut store,
            stream::iter(Vec::<ClientTxEvent>::new()),
            sender,
        )
        .await;

        assert!(result.is_ok());

        let received: Vec<ClientTxStateEvent> = receiver.collect().await;

        assert!(received.is_empty());
        assert!(store.is_empty());
    }
}
