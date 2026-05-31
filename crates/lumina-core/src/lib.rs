pub mod easing;
pub mod events;
pub mod interpolator;
pub mod scene;
pub mod scene_patch;
pub mod timeline;

#[cfg(test)]
mod easing_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod interp_tests;
#[cfg(test)]
mod scene_patch_tests;
#[cfg(test)]
mod stress_tests;
#[cfg(test)]
mod timeline_tests;

pub use events::{EmittedEvent, Event, EventBus, EventOutcome, PlaybackState};
pub use scene::SceneGraph;
pub use scene_patch::{apply_patch, PatchError, PatchOp, ScenePatch};
pub use timeline::Timeline;
