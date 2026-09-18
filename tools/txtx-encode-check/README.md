# txtx-encode-check

Runs txtx's own borsh encoder — the exact crate version surfpool bundles —
against the real program IDLs, with the exact `Value` shapes the publish
runbook produces, and asserts the bytes are identical to a hand-built borsh
encoding. It also reproduces the two failure modes that cost this project
several rounds, so nobody has to rediscover them on a live Surfnet:

- a raw `Value::Addon` pubkey argument panics with `not yet implemented`
- the system program written as `"111…1"` panics with an index out of range

## Run

```bash
# generate the IDLs (anchor build does this too, under target/idl)
cd programs/svi-core        && anchor build && cd ../..
cd programs/svi-hylo-adapter && anchor build && cd ../..

cargo run --manifest-path tools/txtx-encode-check/Cargo.toml -- \
  programs/svi-core/target/idl/svi_core.json \
  programs/svi-hylo-adapter/target/idl/svi_hylo_adapter.json
```

Exit 0 with all `PASS` lines means the runbook's `instruction_args` encode
correctly on that txtx version. Run it after touching either program's
argument structs or `runbooks/publish/main.tx`.

## Why the pins

`txtx-addon-network-svm = 0.3.8` is what surfpool v0.10.8 ships. It does not
compile against newer `txtx-addon-kit` releases — the `CommandImplementation`
trait grew a parameter — so the three txtx crates are pinned together to the
versions in surfpool v0.10.8's lockfile. To check a newer surfpool, bump all
three to the versions in its `Cargo.lock`.
