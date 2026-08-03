# payment-engine

A CLI payment engine that replays a stream of client transactions (deposits,
withdrawals, disputes, resolves, chargebacks) and prints the resulting
per-client account balances.

It's built as a single binary today, but the internal architecture is
designed around a two-layer, shardable pipeline so the same core can scale
out from "one process" to "one host per shard" without changing the
processing logic.

## Usage

```sh
cargo run -- transactions.csv > accounts.csv
```

Input is a CSV file with a header row:

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```

| column | meaning                                                     |
|--------|-------------------------------------------------------------|
| type   | `deposit`, `withdrawal`, `dispute`, `resolve`, `chargeback` |
| client | u16 client id                                               |
| tx     | u32 transaction id                                          |
| amount | present for `deposit`/`withdrawal`, omitted otherwise       |

Output on stdout is a CSV of final account state per client:

```csv
client, available, held, total, locked
1, 1.5, 0, 1.5, false
2, 2.0, 0, 2.0, false
```

(client 2's withdrawal of 3.0 is rejected, insufficient available funds, so
its balance is unaffected.)

## Architecture

Processing is split into two independent, pipelined layers connected by
channels:

```
CSV records ──▶ [ tx layer ] ──▶ tx-state events ──▶ [ account layer ] ──▶ account balances
                sharded by tx_id                     sharded by client_id
```

1. **Transaction layer** (`core::business::process_client_tx_event_stream`)
   owns transaction state, it decides whether a `dispute`/`resolve`/
   `chargeback` refers to a valid, known transaction, keyed by
   `(client_id, tx_id)`. It turns each incoming `ClientTxEvent` into a
   `ClientTxStateEvent`.
2. **Account layer** (`core::business::process_client_tx_state_event_stream`)
   owns account state, applying each `ClientTxStateEvent` to the relevant
   client's `Account` (`available`, `held`, `locked`).

Each layer holds its state in a plain in-memory (`HashMap` in this implementation), 
no external store, no lock contention, which is what lets the engine push a high 
number of transactions per second: as long as a shard's state fits in RAM and 
mutations stay within a single owning task/host, lookups and updates are O(1) 
with no synchronization overhead.

### Why it shards the way it does

The transaction layer is sharded by `tx_id`, and the account layer by
`client_id`. Both are chosen so that all a shard needs to touch is its
own state, nothing else. A transaction's dispute lifecycle is only ever
looked up by its `tx_id`, so all events for a given transaction can be routed
to the same shard and processed without talking to any other shard. Likewise,
an account is only ever mutated by events for its own `client_id`. Because
the two layers key on different fields, they're sharded independently and
connected by a shuffle in between (transaction-layer output is re-keyed by
`client_id` on its way into the account layer).

This is what makes the design horizontally scalable in principle: each shard
is a self-contained unit of state that can live on its own host, with no
cross-shard coordination needed beyond routing events to the right place.

### Current implementation

In this binary, both layers run in-process as independently sharded, async
Tokio tasks rather than separate hosts:

- `n_consumers` (defaults to `available_parallelism() / 2`) transaction-layer
  shards and the same number of account-layer shards are spawned as Tokio
  tasks, each owning its own `HashMap`.
- `RouterSink` (`core::business::router_sink`) is a `futures::Sink`
  combinator that fans a single stream out to per-shard channels using a
  selector closure, `tx_id % n_consumers` for the transaction layer,
  `client_id % n_consumers` for the account layer, so it's the same
  primitive used at both hand-off points.
- Shards communicate via bounded `tokio::sync::mpsc` channels, which also
  provides backpressure from the account layer back to the transaction layer
  and from there back to CSV parsing.
- On completion, each account-layer shard's `HashMap` is drained and
  serialized to stdout as CSV.

Because the sharding key and the routing logic are the only
network/placement-specific pieces, promoting a shard from "task in this
process" to "task on another host" is a matter of swapping the channel
transport (e.g. an in-process `mpsc` for a network queue) behind the same
`Sink`/`Stream` interfaces, the business logic in `core::business` doesn't
change.

## Project layout

```
src/
  core/          domain model + business logic (transport-agnostic)
    model/       ClientTxEvent, ClientTxStateEvent, Account, Amount, ...
    business/    per-layer stream processors, in-memory stores, RouterSink
  inbound/       CSV (de)serialization
    model/       CSV row <-> domain model conversions
  main.rs        CLI wiring: reads CSV, spins up shards, writes CSV
```

## Other design decisions

### `Amount`: fixed-point integer instead of float

`Amount` wraps a `u64` rather than an `f32`/`f64`. The wrapped integer counts
the smallest unit the CSV format supports, ten-thousandths (4 decimal
places), the same idea as storing money as cents, just at finer-than-cent
precision to match the input/output format's `x.xxxx` amounts. `Add`,
`AddAssign` and `SubAssign` are then plain integer arithmetic, so summing
millions of deposits/withdrawals can't accumulate the rounding error a
binary float would introduce, and equality/ordering (used throughout
`Account::apply` to check sufficient funds) stay exact.

`u64` instead of a signed integer is a deliberate constraint, not an
oversight: account balances in this domain are never meant to go negative
(`Account::apply` rejects a withdrawal/dispute/chargeback that would exceed
`available`/`held`), so the type itself rules out a whole class of bugs
where an unchecked balance drifts below zero, that would need to surface as
a subtraction overflow/panic rather than silently produce a negative
balance.

The tradeoff is that (de)serialization can't just be `#[derive]`'d, parsing
and formatting the `x.xxxx` string representation into/out of a scaled `u64`
is hand-written (`inbound/model/amount.rs`, `amount_visitor.rs`), including
rejecting negative input at the parse boundary.

### Correctness: two independent validation layers

Each of the two layers described in [Architecture](#architecture) validates
its own slice of the domain, and neither trusts the other to have done it
right:

- **`TxState` validates that a transaction event is a legal transition for
  that transaction**, independent of any account balance. It's an explicit
  state machine (`core/model/tx_state.rs`) keyed by `(client_id, tx_id)`:

  ```
  Uninitialized ─▶ Deposit ─▶ Dispute ─┬─▶ Resolve (terminal)
                                       └─▶ Chargeback (terminal)
  Uninitialized ─▶ Withdrawal (terminal)
  ```

  `TxState::apply` is a total match over `(state, event)`; anything not
  listed above is an `Err` that leaves the state untouched. This is where
  the assumptions about transaction types that aren't spelled out by the
  input format get encoded as data, rather than scattered `if`s:
  - **Only a `Deposit` can be disputed.** There's no `(Withdrawal,
    Dispute)` arm, a `dispute` against a withdrawal is rejected, since a
    withdrawal has already left the account and there's no held amount to
    reference.
  - **`resolve`/`chargeback` only make sense against a `Dispute`**, both
    are only reachable from the `Dispute` state, so referencing a `tx` that
    was never disputed (or already resolved/charged back) is rejected.
  - **`Resolve` is terminal.** Once a disputed deposit is resolved it moves
    to `TxState::Resolve`, not back to `TxState::Deposit`, so the same
    transaction can't be disputed a second time. (`Chargeback` is likewise
    terminal, and additionally locks the account, see below.)

- **`Account::apply` validates that a state transition is actually
  affordable**, independent of transaction history. Given a
  `TxStateEvent` that `TxState` has already deemed a legal transition, it
  still checks the account has enough `available`/`held` funds to cover it,
  and rejects everything once `locked` is set. This is what stops, e.g., a
  withdrawal for more than the current balance, even though nothing about
  that withdrawal is illegal from `TxState`'s point of view.

Splitting validation this way keeps each check local to the state it needs:
`TxState` never looks at balances, `Account` never looks at transaction
history. That locality is also what makes the sharding in
[Architecture](#architecture) sound, `TxState` only needs the shard keyed by
`tx_id`, `Account` only needs the shard keyed by `client_id`, so neither
check requires reaching across shards.

Rejections from either layer aren't treated as fatal errors: both
`process_client_tx_event_stream` (`TxState` rejections) and
`process_client_tx_state_event_stream` (`Account` rejections) log the
dropped event to stderr and move on, so one malformed or inapplicable event
in the input doesn't abort the run.

## Building & testing

```sh
cargo build --release
cargo test
```

## AI usage

Parts of this project were built with AI assistance (Claude Code). The (almost) 
full transcript can be found at `transcripts`.

Roughly:

- **Unit test generation.** Given an existing function or struct, prompts
  like *"Write unit tests for `FUNCTION`|`STRUCT` in `FILE`"* were used to
  generate the bulk of the test modules (e.g. `Account::apply`,
  `CsvClientTxRecord::try_from`).
- **Boilerplate/snippet generation.** Standard library-adjacent wiring —
  `clap` argument parsing, `csv` reader/writer setup, stream consumption via
  `futures`/`tokio-stream`, was scaffolded from generic prompts like *"Write
  a sample CLI using clap"* or *"Write a sample CSV reader/writer using
  csv"*, then adapted to this project's types.
- **`Amount` type and its (de)serialization.** The fixed-point `Amount(u64)`
  type (4 decimal places, no negatives) and its custom `Serialize`/
  `Deserialize` impls (`inbound/model/amount.rs`, `amount_visitor.rs`), used
  instead of a float to avoid rounding drift, were AI-generated from a prompt 
  describing the desired representation and CSV round-trip behavior, along 
  with the accompanying edge-case tests (leading zeros, trailing zero-padding, 
  too many/few decimal places, negative and malformed input).
- **`RouterSink`.** This one didn't get much design thought beyond the core
  idea: a sink is only ready to accept an item once every downstream sink it
  could route to is ready, everything else follows from that. It was
  generated from a skeleton snippet plus the prompt *"Implement RouterSink
  at `FILE`. Ensure all sinks are ready at `poll_ready`."*, in
  `core/business/router_sink.rs`.
- **This README** was generated from a short description of the project's
  topic, together with the `src` tree and the sample transaction CSVs, then
  reviewed and corrected against the actual code and fixture data.
