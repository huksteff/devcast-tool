//! Which built-in commands `batch` and an AI agent run refuse, and the registry op a command
//! line names.

use super::{Builtin, Cli};
use crate::builtin_groups::{AudioCommand, CaptionsCommand, ClipCommand, ZoomCommand};
use crate::sources;

impl Cli {
    /// Why this command cannot run inside `batch` (it works on one project there): `None` when
    /// it can.
    pub(crate) fn not_for_batch(&self) -> Option<&'static str> {
        match &self.command {
            Builtin::Help { .. } | Builtin::Skill { .. } | Builtin::Defaults { .. } => {
                Some("не работает с проектом")
            }
            Builtin::Program(program) => Some(program.not_for_batch()),
            Builtin::Ls { list, .. } if list == sources::SOURCES || list == super::PROJECTS => {
                Some("не работает с проектом")
            }
            Builtin::New(_) | Builtin::Record { .. } => Some("создаёт новый проект"),
            Builtin::Rename { .. } => {
                Some("даёт всем проектам одно имя; переименуй каждый с --project")
            }
            Builtin::Save(args) if args.as_name.is_some() => {
                Some("даёт всем копиям одно имя; копируй каждый проект с --project")
            }
            Builtin::Close(_)
            | Builtin::Captions {
                command: CaptionsCommand::Stop,
            } => Some("в редакторе открыт один проект; запусти её с --project"),
            Builtin::Batch(_) | Builtin::Style { .. } | Builtin::Trash(_) => {
                Some("сама перечисляет проекты")
            }
            Builtin::Export { .. } | Builtin::Frame(_) | Builtin::Sheet(_) => Some(
                "пишет один файл -o на все проекты; запусти её для каждого проекта с --project",
            ),
            Builtin::Captions {
                command: CaptionsCommand::Export { .. },
            } => Some(
                "пишет один файл -o на все проекты; запусти её для каждого проекта с --project",
            ),
            _ => None,
        }
    }

    /// Why the user's AI agent may not run this command during «Сделать красиво»
    /// (`DEVCAST_GROUP`): the whole run is one undo step that is the user's, and the project as a
    /// whole, the program and files outside it are theirs too. `None` when it may.
    pub(crate) fn not_for_agent(&self) -> Option<&'static str> {
        match &self.command {
            Builtin::Undo { .. } | Builtin::Redo => Some(
                "весь запуск — один шаг, отменяет его пользователь; ошибку исправь новой командой (вырез вернёт только пользователь — скажи об этом в ответе)",
            ),
            Builtin::Save(_)
            | Builtin::Discard
            | Builtin::Open
            | Builtin::Close(_)
            | Builtin::Rename { .. }
            | Builtin::Trash(_)
            | Builtin::New(_)
            | Builtin::Batch(_)
            | Builtin::Record { .. }
            | Builtin::Export { .. }
            | Builtin::Program(_)
            | Builtin::Defaults { .. }
            | Builtin::Skill { .. } => Some("это делает сам пользователь"),
            _ => None,
        }
    }

    /// The words of the registry op this command line names (`clip move c2 --to 1`), so `batch`
    /// can check the name and the arguments once before the projects.
    pub(crate) fn op_words(&self) -> Option<Vec<String>> {
        let (group, words) = match &self.command {
            Builtin::Op(words) => return Some(words.clone()),
            Builtin::Clip {
                command: ClipCommand::Op(words),
            } => ("clip", words),
            Builtin::Zoom {
                command: ZoomCommand::Op(words),
            } => ("zoom", words),
            Builtin::Captions {
                command: CaptionsCommand::Op(words),
            } => ("captions", words),
            Builtin::Audio {
                command: AudioCommand::Op(words),
            } => ("audio", words),
            _ => return None,
        };
        Some([vec![group.to_owned()], words.clone()].concat())
    }
}
