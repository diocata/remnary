//! Local conversation imports, encrypted storage and evidence-backed views.

pub mod analysis;
pub mod model;
pub mod projections;
pub mod reports;
pub mod sources;
pub mod storage;
pub mod web;

// Preserve the public module paths used by existing callers.
pub use analysis::{intelligence, ollama, redaction as privacy, verification};
pub use projections as knowledge;
pub use projections::{feedback as vault_feedback, transcripts as vault};
pub use reports::topics;
pub use sources as local_sources;
pub use sources::{app_snapshot, chatgpt as import};
pub use storage as store;
pub use storage::{
    exclusions, observations as analysis_store, operations, privacy as privacy_store,
    projections as projection_store, security,
};

pub(crate) use analysis::{batches as analysis_batches, reconciliation};
pub(crate) use sources::origins;
