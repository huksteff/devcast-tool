//! `annot key add|rm|copy|move` and the key side of the edit commands: times come from the
//! output video and become key times of the annotation (`keys.rs`).

use dc_core::time::Output;
use dc_core::{Command, CommandError, Diff, Op, OutputTime, Project, TimeArg, TimeDelta};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::commands::position;
use crate::model::{AnnotId, Annotation, Annotations, Kind, fields};
use crate::span::{shown, timeline};

/// The `set` command of a kind: [`edit_body`] of annotation `id` in the key at output `at`, or
/// everywhere without it. The annotation must be of `kind`, else the error is `{id} — {wrong}`.
/// An empty `Diff` when nothing changed.
pub fn edit_kind<B: Serialize + DeserializeOwned + PartialEq>(
    project: &mut Project,
    id: AnnotId,
    kind: Kind,
    wrong: &str,
    at: Option<OutputTime>,
    change: impl Fn(&B) -> Result<B, CommandError>,
) -> Result<Diff, CommandError> {
    let mut state = project.state::<Annotations>()?;
    let index = position(&state, id)?;
    let annotation = &mut state.annotations[index];
    if annotation.kind != kind {
        return Err(CommandError::new(format!("{id} — {wrong}")));
    }
    let at = at.map(|t| key_offset(project, annotation, t)).transpose()?;
    if !edit_body(annotation, at, change)? {
        return Ok(Diff::default());
    }
    project.set_state::<Annotations>(&state)?;
    Ok(Diff::changed(id))
}

/// Key time of output moment `t` for `annotation`: `t` must show the annotation or be the end of
/// it on the output video.
pub fn key_offset(
    project: &Project,
    annotation: &Annotation,
    t: OutputTime,
) -> Result<TimeDelta, CommandError> {
    let timeline = timeline(project)?;
    let id = annotation.id;
    let shown = shown(&timeline, annotation).ok_or_else(|| {
        CommandError::new(format!(
            "{id} не видна в готовом видео (её место вырезано): задай время через annot set {id} --from … --to …"
        ))
    })?;
    let (media, now) = if t == shown.end() {
        timeline.end(t)?
    } else {
        timeline.start(t)?
    };
    let range = annotation.range;
    if media != annotation.media || now < range.start() || now > range.end() {
        return Err(CommandError::new(format!(
            "в {t} аннотации {id} нет: на готовом видео она с {} до {}, ключ ставится внутри",
            shown.start(),
            shown.end()
        )));
    }
    Ok(annotation.offset(now))
}

/// The kind's fields of `annotation` changed by `change`: in the key at `at` (created with what
/// is shown there), or with `None` in the annotation and in every key, so the changed fields
/// become the same all along. Every body of the annotation is written whole through the kind's
/// type `B` (missing fields take its defaults), so keys blend field by field. `false` and the
/// annotation untouched when `change` changes nothing.
pub fn edit_body<B: Serialize + DeserializeOwned + PartialEq>(
    annotation: &mut Annotation,
    at: Option<TimeDelta>,
    change: impl Fn(&B) -> Result<B, CommandError>,
) -> Result<bool, CommandError> {
    let id = annotation.id;
    let broken = |error: serde_json::Error| {
        CommandError::new(format!(
            "поля {id} в проекте испорчены ({error}): удали аннотацию (annot rm {id}) и создай заново"
        ))
    };
    let read = |body: &Map<String, Value>| -> Result<B, CommandError> {
        serde_json::from_value(Value::Object(body.clone())).map_err(broken)
    };
    // The body changed in place; whether `change` did anything to it.
    let edit = |body: &mut Map<String, Value>, apply: bool| -> Result<bool, CommandError> {
        let old = read(body)?;
        if !apply {
            *body = fields(&old).map_err(broken)?;
            return Ok(false);
        }
        let new = change(&old)?;
        *body = fields(&new).map_err(broken)?;
        Ok(new != old)
    };
    let mut new = annotation.clone();
    let mut changed = edit(&mut new.body, at.is_none())?;
    for key in &mut new.keys {
        changed |= edit(&mut key.body, at.is_none())?;
    }
    if let Some(offset) = at {
        let mut shown = new.body_at(offset);
        if edit(&mut shown, true)? {
            new.edit_key(offset).body = shown;
            changed = true;
        }
    }
    if changed {
        *annotation = new;
    }
    Ok(changed)
}

/// Where the keys of `annotation` are on the output video, for error messages.
fn key_times(project: &Project, annotation: &Annotation) -> String {
    if annotation.keys.is_empty() {
        return "ключей у неё нет".to_owned();
    }
    let timeline = timeline(project).ok();
    let times: Vec<String> = annotation
        .keys
        .iter()
        .map(|key| {
            let now = annotation.range.start() + key.at;
            timeline
                .as_ref()
                .and_then(|t| t.map().locate(&annotation.media, now))
                .map(|(_, at)| at)
                .map_or_else(|| "вне показа".to_owned(), |t| t.to_string())
        })
        .collect();
    format!("ключи на готовом видео: {}", times.join(", "))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotKeyAdd {
    /// Annotation to add the key to.
    pub id: AnnotId,
    /// Moment on the output video, inside the annotation; the key takes what is shown there.
    pub at: TimeArg<Output>,
}

impl Command for AnnotKeyAdd {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        let annotation = &mut state.annotations[index];
        let offset = key_offset(project, annotation, self.at.0)?;
        if annotation.key_index(offset).is_some() {
            return Ok(Diff::default());
        }
        annotation.put_key(annotation.snapshot(offset));
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AnnotKeyAdd {
    const NAME: &'static str = "annot key add";
    const ABOUT: &'static str =
        "Поставить ключ аннотации: запомнить положение, размер, поворот и поля вида в этот момент";
    const EXAMPLE: &'static str = "annot key add a3 --at 4.5";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotKeyRm {
    /// Annotation whose key to remove.
    pub id: AnnotId,
    /// Moment of the key on the output video (give or take 1 ms).
    pub at: TimeArg<Output>,
}

impl Command for AnnotKeyRm {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        let annotation = &mut state.annotations[index];
        let offset = key_offset(project, annotation, self.at.0)?;
        let key = annotation.key_index(offset).ok_or_else(|| {
            CommandError::new(format!(
                "у {} нет ключа в {}: {}",
                self.id,
                self.at.0,
                key_times(project, annotation)
            ))
        })?;
        let key = annotation.keys.remove(key);
        if annotation.keys.is_empty() {
            annotation.rect = key.rect;
            annotation.rotation = key.rotation;
            annotation.body = key.body;
        }
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AnnotKeyRm {
    const NAME: &'static str = "annot key rm";
    const ABOUT: &'static str =
        "Удалить ключ аннотации; без последнего ключа аннотация остаётся такой, как в нём";
    const EXAMPLE: &'static str = "annot key rm a3 --at 4.5";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotKeyCopy {
    /// Annotation whose look to copy.
    pub id: AnnotId,
    /// Moment of the key to copy on the output video (give or take 1 ms).
    pub at: TimeArg<Output>,
    /// Where to put the copy on the output video; a key there is replaced.
    pub to: TimeArg<Output>,
    /// Annotation of the same kind to copy into; this one by default.
    pub into: Option<AnnotId>,
}

impl Command for AnnotKeyCopy {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let from = &state.annotations[position(&state, self.id)?];
        let offset = key_offset(project, from, self.at.0)?;
        let mut key = from
            .key_index(offset)
            .map(|i| from.keys[i].clone())
            .ok_or_else(|| {
                CommandError::new(format!(
                    "у {} нет ключа в {}: {}",
                    self.id,
                    self.at.0,
                    key_times(project, from)
                ))
            })?;
        let kind = from.kind;
        let into = self.into.unwrap_or(self.id);
        let index = position(&state, into)?;
        let target = &mut state.annotations[index];
        if target.kind != kind {
            return Err(CommandError::new(format!(
                "ключ {} ({}) нельзя вставить в {into} ({}): ключи копируются только между аннотациями одного вида",
                self.id,
                kind.name(),
                target.kind.name()
            )));
        }
        key.at = key_offset(project, target, self.to.0)?;
        let before = target.keys.clone();
        target.put_key(key);
        if target.keys == before {
            return Ok(Diff::default());
        }
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(into))
    }
}

impl Op for AnnotKeyCopy {
    const NAME: &'static str = "annot key copy";
    const ABOUT: &'static str =
        "Скопировать ключ аннотации в другой момент или в другую аннотацию того же вида";
    const EXAMPLE: &'static str = "annot key copy a3 --at 4.5 --to 7 --into a5";
    const POSITIONAL: &'static [&'static str] = &["id"];
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotKeyMove {
    /// Annotation whose key to move.
    pub id: AnnotId,
    /// Moment of the key on the output video (give or take 1 ms).
    pub at: TimeArg<Output>,
    /// Its new moment on the output video, inside the annotation; a key there is replaced.
    pub to: TimeArg<Output>,
}

impl Command for AnnotKeyMove {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let mut state = project.state::<Annotations>()?;
        let index = position(&state, self.id)?;
        let annotation = &mut state.annotations[index];
        let offset = key_offset(project, annotation, self.at.0)?;
        let key = annotation.key_index(offset).ok_or_else(|| {
            CommandError::new(format!(
                "у {} нет ключа в {}: {}",
                self.id,
                self.at.0,
                key_times(project, annotation)
            ))
        })?;
        let to = key_offset(project, annotation, self.to.0)?;
        // Within `KEY_TOLERANCE` it is the same moment.
        if annotation.key_index(to) == Some(key) {
            return Ok(Diff::default());
        }
        let before = annotation.keys.clone();
        let mut moved = annotation.keys.remove(key);
        moved.at = to;
        annotation.put_key(moved);
        if annotation.keys == before {
            return Ok(Diff::default());
        }
        project.set_state::<Annotations>(&state)?;
        Ok(Diff::changed(self.id))
    }
}

impl Op for AnnotKeyMove {
    const NAME: &'static str = "annot key move";
    const ABOUT: &'static str =
        "Перенести ключ аннотации в другой момент; ключ в новом моменте заменяется";
    const EXAMPLE: &'static str = "annot key move a3 --at 4.5 --to 5.2";
    const POSITIONAL: &'static [&'static str] = &["id"];
}
