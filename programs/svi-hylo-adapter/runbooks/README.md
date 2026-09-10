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

Two things that cost time if you get them wrong: struct arguments are one
object with named fields, and fixed byte arrays like `[u8; 32]` must be arrays
of numbers. A hex string is rejected with `expected vec, found string`.

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

### If step 5 fails with `ContextUnavailable`

That is the design working, not a bug. `hylo-core` refuses to compute when
Hylo's `TotalSolCache` epoch differs from the clock's — the epoch-boundary case
where a naive adapter would publish a stale NAV. On a Surfnet this can happen
if the forked clock and the cached state disagree. Restart the Surfnet to
re-fork at the current slot.

*Fail stale, never fail wrong.*

### Two values to set before any real deployment

`methodology_hash` is zero here. It must be the hash of the frozen
`hylo-xsol-nav-v1` document, since consumers check it to detect a feed's
definition changing under them. And `max_age_slots` is 750 (~5 min); confirm it
against Pyth's actual update cadence.
