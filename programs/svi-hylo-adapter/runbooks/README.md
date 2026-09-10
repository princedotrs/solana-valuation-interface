# svi-hylo-adapter runbooks

[![Surfpool](https://img.shields.io/badge/Operated%20with-Surfpool-gree?labelColor=gray)](https://surfpool.run)

## publish

Deploy both programs and publish a real xSOL NAV quote, in one command.

```bash
# build BOTH programs first -- they are separate Anchor workspaces
cd programs/svi-core          && anchor build
cd ../svi-hylo-adapter        && anchor build

surfpool start                                     # one shell: forks mainnet
surfpool run publish --env localnet --unsupervised # another
```

The two programs live in separate Anchor workspaces, so `svi-core`'s artifacts
are not under this project's `./target`. The runbook passes its keypair, IDL
and binary paths explicitly; the adapter's use the defaults. If you see
`invalid anchor idl location ./target/idl/svi_core.json`, svi-core has not been
built yet.

**`--env localnet` is required.** Signer files are named `signers.<env>.tx`,
and txtx skips any file whose name has three dot-components unless the middle
one matches the selected environment. Without `--env` the signers are silently
not loaded and every action fails with `unable to resolve 'signer.payer'`.
There is no default: surfpool passes `Option<String>` straight through.

`--unsupervised` skips the interactive confirmation prompt. Omit it if you want
to approve each step; keep it for a recording or for CI.

Alternatively, `surfpool start --watch` re-runs the runbook whenever the
programs recompile, and supplies both settings itself.

Against a Surfnet this runs on **forked mainnet**, so the adapter reads Hylo's
actual account bytes. Nothing is mocked and nothing needs cloning by hand —
Surfpool fetches whatever the transaction touches.

### What it does

| Step | Action |
|---|---|
| 1 | Deploy `svi-core`, the notice board |
| 2 | Deploy `svi-hylo-adapter`, the calculator |
| 3 | Create the feed descriptor and its quote account |
| 4 | Register the adapter's PDA as the only key allowed to write |
| 5 | Crank the adapter once — read Hylo, compute NAV, publish |

Step 5 is the whole thesis: a value computed on-chain from verified accounts,
written where anyone can read it. Note it is signed by the **payer**, not the
authority — cranking is permissionless, and the keeper cannot influence what
gets written.

### Runbook style

Instruction blocks are **IDL-driven**. Each one gives `program_idl`,
`instruction_name` and `instruction_args`, then names every account as its own
nested block:

```hcl
instruction {
    program_idl = action.deploy_svi_core.program_idl
    instruction_name = "initialize_feed"
    instruction_args = [{ feed_id = variable.feed_id, ... }]

    authority { public_key = signer.authority.public_key }
    quote     { public_key = variable.quote.pda }
}
```

Accounts not named are derived from the IDL — `system_program`, sysvars, and
PDAs whose seeds the IDL records. `is_signer` always comes from the IDL; only
`is_writable` can be overridden.

Three traps, none guessable from the error text:

**Struct arguments** are one object with named fields.

**Fixed byte arrays** like `[u8; 32]` must be arrays of numbers in
`instruction_args` (a hex string gives `expected vec, found string`) — but the
*same value* used as a `find_pda` seed must be a hex string, because
`get_seeds_from_value` calls `to_le_bytes()` per seed and a 32-number array
blows past the 32-byte limit. That is why `feed_id` and `feed_id_seed` both
exist, with an assertion that they agree.

**Pubkeys in `instruction_args`: the rule comes from txtx's source, not from
its declared types, which are wrong in both directions.**

| Value | Declared type | What it actually is | Form for `instruction_args` |
|---|---|---|---|
| `action.*.program_id` | `Type::string()` | `Value::Addon` of raw bytes (`deploy_program.rs:394`, `SvmValue::pubkey(bytes)`) | wrap in `std::encode_base58` |
| `variable.*.pda` | `Type::addon(SVM_PUBKEY)` | `Value::string(pda.to_string())`, base58 (`functions.rs`, `FindPda::run`) | pass straight through |
| system program | — | a literal | `"0x" + 64 hex zeros`, **not** `"111…1"` |

Why raw bytes cannot be passed: txtx 0.3.8's borsh encoder matches on the
value before the IDL type:

```rust
Value::Addon(addon_data) => return borsh_encode_bytes_to_idl_type(...),
```

and that function implements only `IdlType::U8`, hitting a `todo!()` for
`IdlType::Pubkey`. The CLI panics with `not yet implemented` at
`idl/mod.rs:539` and never says which argument did it. A `String` instead
reaches `IdlType::Pubkey => SvmValue::to_pubkey`, which works.

Why not `encode_base58` everything: `std::encode_base58` takes an Addon's
bytes but *hex-decodes* a string, so wrapping an already-base58 `.pda` fails
on its first non-hex character — `Invalid character 'z' at position 3` on
`288zBibx…`, the adapter authority PDA.

Why not `"11111111111111111111111111111111"`: `SvmValue::to_pubkey` tries hex
before base58, and 32 ones are valid hex. They decode to 16 bytes, and
`hex[0..32]` then panics with an index out of range.

Account blocks and `find_pda`'s program argument are unaffected: both go
through `SvmValue::to_pubkey` directly, which accepts either form.

This is checked offline by `tools/txtx-encode-check`, which runs txtx's own
encoder against the real IDLs with these exact value shapes and asserts the
bytes match a hand-built borsh encoding. Run it after touching either program's
argument structs or this runbook's `instruction_args`.

### Addresses

The three Hylo accounts are derived from the SDK rather than typed by hand:

```bash
cargo run --manifest-path ../../tools/nav-check/Cargo.toml -- --addresses
```

Everything else is a PDA derived inside the runbook, so a seed change in either
program surfaces as a failed transaction rather than a silently wrong account.

### Reading the result

The runbook outputs `quote_account`. Read those 320 bytes and you have the
value, its bounds, the slot it was observed at, when it expires, and a hash of
the methodology that produced it.

For a decoded view plus an independent off-chain cross-check:

```bash
cargo run --manifest-path ../../tools/svi-e2e/Cargo.toml
```

### Deploy through the runbook, not the Solana CLI

`svm::deploy_program` is the only deploy path this project uses against a
Surfnet. `solana program deploy` does not work there and fails in two ways that
are easy to misread:

```
Should return a valid tpu client: Custom("Failed find any cluster node info
for upcoming leaders, timeout: 20s.")

Error: Account allocation failed: ... Instruction 2: invalid instruction data
Program BPFLoaderUpgradeab1e... failed: invalid instruction data
```

The CLI wants TPU and gossip, which an RPC-only SVM does not serve, and it
sends loader instructions Surfpool rejects. The dangerous part is what happens
next: a failed redeploy leaves the *previous* binary in place and running, so
a code change appears to have had no effect. If a program's behaviour did not
change after a rebuild, check that the deploy actually succeeded before
suspecting the code.

### If step 5 fails with a refusal

That is the design working, not a bug. Which refusal matters, because the
remedies differ:

| Error | Meaning | Remedy |
|---|---|---|
| `HyloCacheStale` (6011) | Hylo's `TotalSolCache` epoch ≠ the clock's | somebody must call Hylo's `update_lst_prices` for the new epoch |
| `OracleStale` (6012) | Pyth publish time or posted slot outside Hylo's `oracle_interval_secs` | a Pyth push |
| `OracleConfidenceTooWide` (6013) | Pyth confidence wider than Hylo's own tolerance | wait for the market to settle |
| `ContextUnavailable` (6005) | anything `hylo-core` does not distinguish | read the log line |

Every refusal logs the epochs, slots and timestamps behind it, since the code
alone does not say by how much a value missed.

`OracleStale` is the one to expect on a Surfnet, and it is an artefact of the
fork rather than anything about Hylo. Hylo's oracle window is **10 seconds**,
and hylo-core checks both `unix_timestamp <= publish_time + 10` and
`posted_slot <= slot <= posted_slot + 25`. A Surfnet clones the Pyth account
once and never updates it, its slot counter drifts from mainnet's (fixed 400 ms
ticks vs. mainnet's real cadence) while its timestamp tracks wall time, and
`surfnet_timeTravel` only moves forward, dragging the timestamp with the slot.
On any Surfnet older than a few minutes those two constraints cannot both hold.

txtx has no clock cheatcodes, so this runbook cannot fix that. The adapter's
live test can, and it is the reference path for step 5:

```bash
cd programs/svi-hylo-adapter
cargo test --test surfnet -- --nocapture
```

It re-clones Hylo's three accounts from mainnet, pauses the Surfnet clock,
pins the Clock sysvar to the snapshot's own `posted_slot + 1` /
`publish_time + 1` for the one instruction that reads it, and resumes. That is
the refresh evaluated exactly as a fork taken at `posted_slot` would evaluate
it; no Hylo or Pyth byte is altered, and a NAV computed from a hand-edited
oracle would prove nothing.

*Fail stale, never fail wrong.*

### Two values to set before any real deployment

`methodology_hash` is zero here. It must be the hash of the frozen
`hylo-xsol-nav-v1` document, since consumers check it to detect a feed's
definition changing under them. And `max_age_slots` is 750 (~5 min); confirm it
against Pyth's actual update cadence.
