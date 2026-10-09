//! Offline authoring of standalone native `.beat` scenes.
mod controller;
#[cfg(feature = "offline")]
mod prediction;
#[cfg(feature = "offline")]
mod scene;

pub use controller::{ControlFrame, Controller};
#[cfg(feature = "offline")]
pub use prediction::{Prediction, SourceHints};
#[cfg(feature = "offline")]
pub use scene::{
    AdaptationMetadata, EvidenceTarget, SynthesisRequest, SynthesisResult, synthesize,
};
