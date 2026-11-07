# Pipeline declaration (Thief harness)

## Implementation (Rust)

This agent is a Rust port of the Python reference (`submit/build-agent`, tag v14). Each `harness/<module>.py` is
`src/<module>.rs`. Every model request it sends is meant to be identical to the reference's for the same case;
`PORTING.md` holds the contract.

