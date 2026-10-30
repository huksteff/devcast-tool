//! `devcast set zoom.preset=smooth captions.size=36`: assignments `<key>.<arg>=<value>` → one step
//! per settings key, the arguments of the op that owns the key ([`dc_core::Op::SETTINGS`]). The op
//! itself checks the values, so `set` accepts exactly what its command line would.

use crate::argv::parse_args;
use crate::error::OpsError;
use crate::plan::Step;
use crate::registry::Registry;

const HINT: &str = "нужно настройка.параметр=значение, например zoom.preset=smooth; настройки и их команды — devcast help set";

/// Steps of `assignments`, one per key in the order the keys first appear. A value is read like
/// the value of `--<arg>` on the op's command line.
pub fn settings_steps(registry: &Registry, assignments: &[String]) -> Result<Vec<Step>, OpsError> {
    if assignments.is_empty() {
        return Err(OpsError::Usage(HINT.to_owned()));
    }
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    for assignment in assignments {
        let parsed = assignment
            .split_once('=')
            .and_then(|(name, value)| Some((name.split_once('.')?, value)));
        let Some(((key, arg), value)) = parsed
            .filter(|((key, arg), _)| !key.is_empty() && !arg.is_empty() && !arg.starts_with('-'))
        else {
            return Err(OpsError::Usage(format!("«{assignment}»: {HINT}")));
        };
        let flag = format!("--{arg}={value}");
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, flags)) => flags.push(flag),
            None => groups.push((key, vec![flag])),
        }
    }
    groups
        .into_iter()
        .map(|(key, flags)| {
            let op = registry.settings_op(key)?;
            let args = parse_args(op, &flags).map_err(|e| prefixed(key, e))?;
            Ok(Step::new(op.name, args))
        })
        .collect()
}

fn prefixed(key: &str, error: OpsError) -> OpsError {
    match error {
        OpsError::Usage(m) => OpsError::Usage(format!("настройки {key}: {m}")),
        OpsError::Failed(m) => OpsError::Failed(format!("настройки {key}: {m}")),
    }
}

/// The commands of [`settings_steps`], ready for [`crate::apply_commands`]; a bad step is named
/// by its number.
pub fn settings_commands(
    registry: &Registry,
    assignments: &[String],
) -> Result<Vec<Box<dyn dc_core::Command>>, OpsError> {
    crate::plan::build(registry, &settings_steps(registry, assignments)?)
}

#[cfg(test)]
mod tests {
    use dc_core::{Command, CommandError, Diff, History, Op, OpInfo, Project};
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct Look {
        /// Font size, px.
        size: Option<f64>,
        /// Font family.
        font: Option<String>,
        /// Smallest gap, seconds.
        min_gap: Option<f64>,
    }

    impl Command for Look {
        fn name(&self) -> &'static str {
            Self::NAME
        }
        fn apply(&self, project: &mut Project) -> Result<Diff, CommandError> {
            if self.size.is_some_and(|s| s > 9.0) {
                return Err(CommandError::new("размер не больше 9"));
            }
            let value =
                json!({"settings": {"size": self.size, "font": self.font, "gap": self.min_gap}});
            project.effects.insert("look".to_owned(), value);
            Ok(Diff::note("вид"))
        }
    }

    impl Op for Look {
        const NAME: &'static str = "look style";
        const ABOUT: &'static str = "Вид";
        const EXAMPLE: &'static str = "look style --size 2";
        const SETTINGS: Option<&'static str> = Some("look");
    }

    fn registry() -> Registry {
        Registry::new(vec![OpInfo::of::<Look>()], Vec::new()).unwrap()
    }

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn assignments_become_the_settings_op_of_their_key() {
        let steps = settings_steps(
            &registry(),
            &words(&["look.size=2", "look.font=123", "look.min-gap=0.5"]),
        )
        .unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].op, "look style");
        assert_eq!(
            serde_json::Value::Object(steps[0].args.clone()),
            json!({"size": 2, "font": "123", "min_gap": 0.5})
        );
    }

    #[test]
    fn the_op_checks_the_values() {
        let commands = settings_commands(&registry(), &words(&["look.size=12"])).unwrap();
        let mut project = Project::default();
        let error = History::default()
            .apply_batch(&mut project, "set", &[commands[0].as_ref()])
            .unwrap_err();
        assert!(error.to_string().contains("не больше 9"), "{error}");
    }

    #[test]
    fn wrong_assignments_say_what_to_write() {
        let registry = registry();
        for (bad, hint) in [
            ("look.size", "настройка.параметр=значение"),
            ("size=2", "настройка.параметр=значение"),
            ("look.=2", "настройка.параметр=значение"),
            ("zoom.size=2", "нет настроек «zoom»: есть look"),
        ] {
            let error = settings_steps(&registry, &words(&[bad])).unwrap_err();
            assert!(matches!(error, OpsError::Usage(_)), "{bad}");
            assert!(error.to_string().contains(hint), "{bad}: {error}");
        }
        let error = settings_steps(&registry, &words(&["look.size=1", "look.size=2"]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("указан дважды"), "{error}");
        let error = settings_commands(&registry, &words(&["look.color=red"]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("color"), "{error}");
        assert!(settings_steps(&registry, &[]).is_err());
    }

    #[test]
    fn settings_keys_are_unique() {
        let error = Registry::new(vec![OpInfo::of::<Look>(), OpInfo::of::<Look>()], Vec::new())
            .unwrap_err();
        assert!(error.contains("ключ настроек «look» уже занят"), "{error}");
    }
}
