//! Offline migration participant. Runtime workflow semantics live in workflow/.
#[path = "../../../../migration/workflow-store-rust-v1/participant.rs"]
mod participant;
fn main() {
    participant::main();
}
