//! The look of a project: `set <настройка>.<параметр>=<значение> …` through the settings ops of the
//! effects (`dc_ops::settings_commands`, each op checks its values) and `style copy`, which puts
//! the settings of one project (`dc_project::Defaults::restyle`) into others.

use std::path::PathBuf;

use dc_core::{Command, CommandError, Diff, Project};
use dc_project::Defaults;

use crate::batch;
use crate::edit::{self, Ctx};
use crate::error::CliError;
use crate::find::find_project;
use crate::run::Answer;
use crate::workspace::Workspace;

/// `devcast set zoom.preset=smooth captions.size=36`: one undo step named `set`.
pub(crate) fn set(ctx: &Ctx<'_>, assignments: &[String]) -> Result<Answer, CliError> {
    let commands = dc_ops::settings_commands(ctx.registry, assignments)?;
    let commands: Vec<&dyn Command> = commands.iter().map(AsRef::as_ref).collect();
    let mut workspace = ctx.open(true)?;
    edit::apply_commands(ctx, &mut workspace, "set", &commands)
}

#[derive(Debug, clap::Args)]
pub(crate) struct CopyArgs {
    #[arg(long, help = "Проект, чей стиль копировать")]
    from: PathBuf,
    #[arg(
        long,
        required = true,
        num_args = 1..,
        help = "Проекты, куда копировать: папки *.dcproj, можно со звёздочкой в имени (*.dcproj)"
    )]
    to: Vec<String>,
}

/// `devcast style copy --from a.dcproj --to *.dcproj`.
pub(crate) fn copy(ctx: &Ctx<'_>, args: &CopyArgs) -> Result<Answer, CliError> {
    if ctx.globals.project.is_some() {
        return Err(CliError::Usage(
            "у style copy проекты задают --from и --to, без --project".to_owned(),
        ));
    }
    let source = find_project(Some(&args.from), ctx.cwd)?;
    let name = source.name().to_owned();
    let style = Defaults::from_project(&Workspace::open(source, false, true)?.project);
    let targets = batch::projects(ctx.cwd, &args.to)?;
    Ok(batch::each(ctx, &targets, |ctx| {
        let restyle = Restyle {
            style: style.clone(),
            from: name.clone(),
        };
        let mut workspace = ctx.open(true)?;
        edit::apply_commands(ctx, &mut workspace, Restyle::NAME, &[&restyle])
    }))
}

/// Settings of every effect become those of the project `from`; content stays.
#[derive(Debug)]
struct Restyle {
    style: Defaults,
    from: String,
}

impl Restyle {
    const NAME: &'static str = "style copy";
}

impl Command for Restyle {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
        let before = Defaults::from_project(project);
        self.style.restyle(project);
        let after = Defaults::from_project(project);
        let changed: Vec<&str> = before
            .all()
            .keys()
            .chain(after.all().keys())
            .filter(|id| before.settings(id) != after.settings(id))
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(match changed.is_empty() {
            true => Diff::default(),
            false => Diff::note(format!("стиль из «{}»: {}", self.from, changed.join(", "))),
        })
    }
}
