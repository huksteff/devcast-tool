//! `image set`: the picture of an image annotation. Not an op of the registry: the file is copied
//! into the project and read by `devcast image set` (dc-cli), which then applies this command.

use dc_core::{Command, CommandError, Diff, OutputTime, Project};
use dc_fx_annotations::{AnnotId, Annotations, Kind, edit_body, key_offset};

use crate::model::{ImageBody, PictureFile};

#[derive(Debug, Clone, PartialEq)]
pub struct ImageSet {
    pub id: AnnotId,
    /// The picture, already in the project.
    pub picture: PictureFile,
    /// Moment on the output video: the picture changes in the key there (created with what is
    /// shown). Without it, in the annotation and in every key.
    pub at: Option<OutputTime>,
}

impl ImageSet {
    pub const NAME: &'static str = "image set";
}

impl Command for ImageSet {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let ids: Vec<AnnotId> = state.annot_ids().collect();
        let annotation = state
            .annotations
            .iter_mut()
            .find(|a| a.id == self.id)
            .ok_or_else(|| CommandError::not_found(self.id, ids))?;
        if annotation.kind != Kind::Image {
            return Err(CommandError::new(format!(
                "{} — не картинка: image set меняет только аннотации image (devcast ls annots покажет виды, devcast annot add image создаст новую)",
                self.id
            )));
        }
        let at = match self.at {
            Some(t) => Some(key_offset(project, annotation, t)?),
            None => None,
        };
        let change = |_: &ImageBody| {
            Ok(ImageBody {
                picture: Some(self.picture.clone()),
            })
        };
        if !edit_body(annotation, at, change)? {
            return Ok(Diff::default());
        }
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}
