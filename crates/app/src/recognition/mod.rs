//! Speech recognition in a process of its own (owner's decisions 2026-10-29 «вариант 2» and
//! 2026-10-31 «вариант А», docs/decisions/018-recognition-process.md): whisper.cpp cannot be
//! stopped inside a process, and the app window must not link Whisper and Vulkan at all, so
//! the editor starts `devcast --captions-worker` (the CLI exe beside this one) and cancels by
//! killing it. The worker runs the path of `devcast captions gen` and answers in lines
//! (`dc_ops::recognition`); the editor applies the cues as `CaptionsGen`, one undo step.

pub mod job;

#[cfg(test)]
pub(crate) mod job_tests;

pub use dc_ops::recognition::{Message, Request, Speech};
