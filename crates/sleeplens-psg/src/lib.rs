//! Read labelled polysomnography so SleepLens accuracy can be *measured*.
//!
//! Two formats cover what is needed to test a breathing-only stage estimate:
//!
//! * **EDF/EDF+** — the container used by Sleep-EDF, SHHS, MESA and most
//!   clinical systems. [`edf`] parses the header and selected channels; [`annotations`]
//!   reads EDF+ `TAL` annotation channels, which is where hypnograms live in
//!   Sleep-EDF.
//! * **Compumedics Profusion XML** — the sidecar format NSRR ships for MESA and
//!   SHHS, one `Sleep_Stage` element per 30-second epoch.
//!
//! The point is not to build a general polysomnography toolkit. It is to get
//! two aligned arrays: respiration samples and a stage per 30-second epoch.
//! [`night`] combines them and maps the five-class AASM/R&K labels onto
//! SleepLens's four classes.
//!
//! No file is written, no network is used, and nothing here is compiled into
//! the desktop app: it is development tooling.

pub mod annotations;
pub mod edf;
pub mod metrics;
pub mod night;
pub mod profusion;
pub mod rec;
pub mod runner;

pub use edf::{Edf, EdfChannel, EdfError};
pub use night::{LabelledNight, StageLabel};
pub use profusion::ProfusionError;
pub use runner::{discover, evaluate, load, Evaluation, Layout, NightFiles};
