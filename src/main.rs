use crate::core::business::RouterSink;
use crate::core::business::{process_client_tx_event_stream, process_client_tx_state_event_stream};
use crate::core::model::{Account, ClientID, ClientTxEvent, ClientTxStateEvent, TxID, TxState};
use crate::inbound::model::{CsvAccount, CsvClientTxRecord};
use anyhow as ah;
use clap::Parser;
use futures::SinkExt;
use std::collections::HashMap;
use std::num::NonZero;
use std::thread::available_parallelism;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::PollSender;

mod core;
mod inbound;

type TxStateStore = HashMap<(ClientID, TxID), TxState>;
type AccountStore = HashMap<ClientID, Account>;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    transactions_file: String,
}

fn filter_map_record(result: Result<CsvClientTxRecord, csv::Error>) -> Option<ClientTxEvent> {
    let record: CsvClientTxRecord = result
        .inspect_err(|err| eprintln!("Failed to deserialize csv record: {:?}", err))
        .ok()?;

    let event: ClientTxEvent = ClientTxEvent::try_from(record)
        .inspect_err(|err| eprintln!("Failed to interpret csv record: {:?}", err))
        .ok()?;

    Some(event)
}

fn setup_stream_consumers(
    n_consumers: NonZero<usize>,
) -> ah::Result<(
    RouterSink<PollSender<ClientTxEvent>, impl Fn(&ClientTxEvent) -> usize>,
    Vec<JoinHandle<TxStateStore>>,
    Vec<JoinHandle<AccountStore>>,
)> {
    // Transaction layer
    let (tx_senders, tx_receivers): (Vec<_>, Vec<_>) = (0..n_consumers.get())
        .map(|_| {
            let (s, r) = mpsc::channel::<ClientTxEvent>(1_000);
            (PollSender::new(s), ReceiverStream::new(r))
        })
        .unzip();

    let tx_senders_len = tx_senders.len();
    let tx_router = RouterSink::new(tx_senders, move |e: &ClientTxEvent| {
        e.tx_id as usize % tx_senders_len
    });

    // Client layer
    let (tx_state_senders, tx_state_receivers): (Vec<_>, Vec<_>) = (0..n_consumers.get())
        .map(|_| {
            let (s, r) = mpsc::channel::<ClientTxStateEvent>(1_000);
            (PollSender::new(s), ReceiverStream::new(r))
        })
        .unzip();

    let tx_state_senders_len = tx_state_senders.len();
    let tx_state_router = RouterSink::new(tx_state_senders, move |e: &ClientTxStateEvent| {
        e.client_id as usize % tx_state_senders_len
    });

    // TxEvent consumers
    let process_client_tx_event_handlers: Vec<_> = tx_receivers
        .into_iter()
        .map(|tx_receiver| {
            let tx_state_router = tx_state_router.clone();

            let mut client_tx_state_store = HashMap::new();

            tokio::spawn(async move {
                if let Err(err) = {
                    process_client_tx_event_stream(
                        &mut client_tx_state_store,
                        tx_receiver,
                        tx_state_router,
                    )
                    .await
                } {
                    eprintln!("Error while processing tx_event: {:?}", err);
                }

                client_tx_state_store
            })
        })
        .collect();

    drop(tx_state_router);

    // TxStateEvent consumers
    let process_client_tx_state_event_handlers: Vec<_> = tx_state_receivers
        .into_iter()
        .map(|tx_state_receiver| {
            let mut client_account_store = HashMap::new();

            tokio::spawn(async move {
                process_client_tx_state_event_stream(&mut client_account_store, tx_state_receiver)
                    .await;

                client_account_store
            })
        })
        .collect();

    Ok((
        tx_router,
        process_client_tx_event_handlers,
        process_client_tx_state_event_handlers,
    ))
}

fn write_output(client_account_store_collection: &[HashMap<ClientID, Account>]) -> ah::Result<()> {
    let mut wtr = csv::Writer::from_writer(std::io::stdout());

    for client_account_store in client_account_store_collection.iter() {
        for (client, account) in client_account_store.iter() {
            let static_account: CsvAccount = (*client, *account).into();

            wtr.serialize(static_account)?;
        }
    }

    wtr.flush()?;

    Ok(())
}

#[tokio::main]
async fn main() -> ah::Result<()> {
    let Args { transactions_file } = Args::parse();

    let transactions_file = std::fs::File::open(transactions_file)?;

    let mut transaction_deserializer = csv::ReaderBuilder::new()
        .delimiter(b',')
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(transactions_file);

    let n_consumers: NonZero<usize> = available_parallelism()
        .ok()
        .and_then(|n| NonZero::new(usize::max(1, n.get() / 2)))
        .unwrap_or(unsafe { NonZero::new_unchecked(1) });

    let (mut tx_router, process_client_tx_event_handlers, process_client_tx_state_event_handlers) =
        setup_stream_consumers(n_consumers)?;

    for client_tx_event in transaction_deserializer
        .deserialize()
        .filter_map(filter_map_record)
    {
        tx_router.send(client_tx_event).await?;
    }
    drop(tx_router);

    for h in process_client_tx_event_handlers.into_iter() {
        h.await?;
    }

    let mut client_account_store_collection = Vec::new();

    for h in process_client_tx_state_event_handlers.into_iter() {
        let client_account_store = h.await?;

        client_account_store_collection.push(client_account_store);
    }

    write_output(&client_account_store_collection)?;

    Ok(())
}
