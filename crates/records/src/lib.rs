//! What Lumenna tells a person, as records, built from a snapshot alone.
//!
//! The rows a list shows and the actions each offers, the questions those ask, the forms'
//! fields and the words of every screen: everything here is a function of what the store
//! holds and what the person sent, and opens nothing. The host (`lumenna-host`) returns these
//! from its operations, for `lum`, its `--json` and `lum rpc`; the app (`lumenna-app`) builds
//! its view models from them over the snapshot it holds. One definition, so a list said by the
//! command line and the same list on a phone cannot drift apart.

pub mod actions;
pub mod day;
pub mod error;
pub mod fields;
pub mod form;
pub mod history;
pub mod lists;
pub mod places;
pub mod resolve;
pub mod tree;
pub mod types;
pub mod words;

pub use actions::{QUERY, go_to_day_question, length_question, new_filter_questions, not_offered, Action, ActionKind, Answer, Choice, Choices, Question, Subject};
pub use error::{LumennaError, Result};
pub use fields::{
    unsayable_repeat_note, FieldKind, FormField, PairingWords, ProposalWords, block_form, pairing_words, proposal_words, sentence_case,
    task_form,
};
pub use form::{parse_minutes, parse_weight, priorities, BlockDefaults, BlockFields, TaskFields, block_defaults, block_edit, block_fields, day_block_fields, new_block, label_reference, project_reference, sitting_status, task_edit, task_fields};
pub use history::{TaskHistory, UrgencyTerm};
pub use types::*;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
