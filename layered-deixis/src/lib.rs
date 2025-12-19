#![doc(
    html_logo_url = "https://raw.githubusercontent.com/storyscript/layered-nlp/main/assets/layered-nlp.svg",
    issue_tracker_base_url = "https://github.com/storyscript/layered-nlp/issues/"
)]

//! Deixis detection for layered-nlp.
//!
//! This crate provides domain-agnostic types and resolvers for detecting
//! deictic expressions - words whose meaning depends on context.
//!
//! ## Deictic Categories
//!
//! - **Person**: Pronouns referring to participants (I, you, we, they)
//! - **Place**: Spatial references (here, there, elsewhere)
//! - **Time**: Temporal references (now, then, today, tomorrow)
//! - **Discourse**: References to parts of the discourse (however, therefore)
//! - **Social**: Social relationship markers (honorifics, formal/informal)
//!
//! ## Usage
//!
//! ```ignore
//! use layered_nlp::create_line_from_string;
//! use layered_deixis::{PersonPronounResolver, PlaceDeicticResolver, DeicticReference};
//!
//! let line = create_line_from_string("I will meet you there tomorrow.");
//! let line = line
//!     .run(&PersonPronounResolver)
//!     .run(&PlaceDeicticResolver);
//!
//! for (range, deictic) in line.attrs_by::<DeicticReference>() {
//!     println!("{}: {:?}", deictic.surface_text, deictic.category);
//! }
//! ```
//!
//! ## Architecture
//!
//! This crate defines:
//! - Core types (`DeicticReference`, `DeicticCategory`, `DeicticSubcategory`)
//! - Simple word-list resolvers that output `DeicticReference` directly
//!
//! Domain-specific crates (like `layered-contracts`) can:
//! - Use these resolvers to fill gaps in their detection
//! - Create mapping resolvers that convert their domain-specific types
//!   to `DeicticReference` (respecting Rust's orphan rules)

mod discourse_marker;
mod person_pronoun;
mod place_deictic;
mod temporal_deictic;
mod types;

pub use discourse_marker::DiscourseMarkerResolver;
pub use person_pronoun::PersonPronounResolver;
pub use place_deictic::PlaceDeicticResolver;
pub use temporal_deictic::SimpleTemporalResolver;
pub use types::{
    DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory, ResolvedReferent,
};
