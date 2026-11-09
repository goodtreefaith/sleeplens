//! Offline AS2X parsing and app-ready night assembly.
//!
//! Low-level parsers accept byte slices. Use [`CardSource`] to import a card
//! through the assembly API; filesystem access is optional via the `fs` feature.

pub mod assembly;
pub mod bkp;
pub mod dat;
pub mod detect;
pub mod model;
pub mod score;
pub mod settings;
pub mod sleep_stages;
pub mod source;
pub mod time;
pub mod tips;

pub use assembly::*;
pub use detect::detect_as2x;
pub use model::*;
pub use score::{refresh_score, sleep_score};
pub use sleep_stages::{SleepStage, SleepStageEstimate, StageMinutes};
pub use source::*;
pub use tips::{night_tips, period_tips};

/// Parser namespace for callers selecting a card family explicitly.
pub mod parsers {
    pub mod as2x {
        pub use crate::bkp::*;
        pub use crate::dat::*;
        pub use crate::detect::*;
        pub use crate::settings::*;
        pub use crate::time::*;
    }
}
