//! `image set`: the picture of an image annotation. Not an op of the registry: the file is copied
//! into the project and read by `devcast image set` (dc-cli), which then applies this command.

use dc_core::{Command, CommandError, Diff, OutputTime, Project};
use dc_fx_annotations::{AnnotId, Kind, edit_kind};

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
        let wrong = "не картинка: image set меняет только аннотации image (devcast ls annots покажет виды, devcast annot add image создаст новую)";
        let change = |_: &ImageBody| {
            Ok(ImageBody {
                picture: Some(self.picture.clone()),
            })
        };
        edit_kind(project, self.id, Kind::Image, wrong, self.at, change)
    }
}
