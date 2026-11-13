//! All ops and effects of DevCast: the one registry of the CLI and the editor, so the same command
//! gives the same project in both, and the pictures of the effects in the frame. A new effect
//! crate adds its `ops()` and `effect()` here, and its `look::Picture` to `LOOKS` when it draws.

use dc_core::{PixelSize, Project, StateError};
use dc_fx_annotations::Kind;
use dc_fx_annotations::look::Read;
use dc_render::look::Prepare;

use crate::error::OpsError;
use crate::registry::Registry;

/// The pictures of the effects in frame order (`Evaluator`): a later one draws over an earlier
/// one of the same band; the callout hangs on the pointer the cursor leaves, so it comes after it.
/// The order is the frames' and must not change without a reason in the commit.
pub(crate) static LOOKS: [&dyn Prepare; 7] = [
    &dc_fx_cut::look::Picture,
    &dc_fx_zoom::look::Picture,
    &dc_fx_annotations::look::Picture(&KINDS),
    &dc_fx_cursor::look::Picture,
    &dc_fx_callout::look::Picture,
    &dc_fx_captions::look::Picture,
    &dc_fx_keys::look::Picture,
];

/// The kinds of annotations and how each reads its fields for the frame.
static KINDS: [(Kind, Read); 5] = [
    (Kind::Text, dc_fx_text::look::READ),
    (Kind::Shape, dc_fx_shape::look::READ),
    (Kind::Image, dc_fx_image::look::READ),
    (Kind::Drawing, dc_fx_drawing::look::READ),
    (Kind::Blur, dc_fx_blur::look::READ),
];

/// The output frame of `project`, px (the crop); none without clips.
pub(crate) fn canvas(project: &Project) -> Result<Option<PixelSize>, StateError> {
    dc_fx_crop::look::canvas(project)
}

impl Registry {
    pub fn devcast() -> Result<Self, OpsError> {
        let ops = [
            dc_fx_cut::ops(),
            dc_fx_crop::ops(),
            dc_fx_zoom::ops(),
            dc_fx_cursor::ops(),
            dc_fx_callout::ops(),
            dc_fx_keys::ops(),
            dc_fx_captions::ops(),
            dc_fx_annotations::ops(),
            dc_fx_text::ops(),
            dc_fx_shape::ops(),
            dc_fx_drawing::ops(),
            dc_fx_blur::ops(),
            dc_fx_audio_tracks::ops(),
        ]
        .concat();
        let effects = vec![
            dc_fx_crop::effect(),
            dc_fx_zoom::effect(),
            dc_fx_cursor::effect(),
            dc_fx_callout::effect(),
            dc_fx_keys::effect(),
            dc_fx_captions::effect(),
            dc_fx_annotations::effect(),
            dc_fx_audio_tracks::effect(),
        ];
        Self::new(ops, effects).map_err(|e| {
            OpsError::Failed(format!(
                "внутренняя ошибка: реестр команд собран неверно ({e}). Это баг, сообщи о нём"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_op_follows_the_rules() {
        let registry = super::Registry::devcast().unwrap();
        assert!(registry.ops().len() >= 10);
    }

    #[test]
    fn every_kind_of_annotation_has_a_picture() {
        for kind in dc_fx_annotations::lane::KINDS {
            assert!(super::KINDS.iter().any(|(k, _)| *k == kind), "{kind:?}");
        }
    }
}
