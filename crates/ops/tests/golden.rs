//! Reference frames of the project's scene (decision 012), one test binary `golden` for
//! `cargo xtask golden update "<почему>"`: crop and aspect (5.16), zoom (5.17–5.19), cursor (5.20–5.23),
//! callout (5.25), keys (5.26), captions (5.28), annotation animations (5.30), text annotations (5.32).

#[path = "golden/callout.rs"]
mod callout;
#[path = "golden/captions.rs"]
mod captions;
mod common;
#[path = "golden/crop.rs"]
mod crop;
#[path = "golden/cursor.rs"]
mod cursor;
#[path = "golden/keys.rs"]
mod keys;
#[path = "golden/motion.rs"]
mod motion;
#[path = "golden/text.rs"]
mod text;
#[path = "golden/zoom.rs"]
mod zoom;
