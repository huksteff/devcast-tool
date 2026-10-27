//! Dependency rules between workspace crates (see docs/ARCHITECTURE.md, «Правила зависимостей»).

use crate::metadata::Workspace;
use crate::run::Outcome;

/// Architectural layer of a crate. A crate may depend only on lower layers,
/// with the exceptions listed in [`allowed`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Layer {
    Core,
    Base,
    Effect,
    Ops,
    Top,
}

/// Crates of the base layer; they may also depend on each other.
const BASE: [&str; 10] = [
    "dc-project",
    "dc-media",
    "dc-audio",
    "dc-render",
    "dc-encode",
    "dc-transcribe",
    "dc-capture",
    "dc-ipc",
    "dc-ui-kit",
    "dc-update",
];

/// The only non-effect crates an effect crate may use.
const EFFECT_MAY_USE: [&str; 3] = ["dc-core", "dc-render", "dc-ui-kit"];

/// Annotation kinds may additionally use the shared annotation crate.
const ANNOTATION_KINDS: [&str; 5] = [
    "dc-fx-text",
    "dc-fx-shape",
    "dc-fx-image",
    "dc-fx-drawing",
    "dc-fx-blur",
];

/// Tooling outside the product; not checked.
const UNCHECKED: [&str; 1] = ["xtask"];

fn layer(name: &str) -> Option<Layer> {
    match name {
        "dc-core" => Some(Layer::Core),
        "dc-ops" => Some(Layer::Ops),
        "dc-cli" | "dc-app" => Some(Layer::Top),
        _ if BASE.contains(&name) => Some(Layer::Base),
        _ if name.starts_with("dc-fx-") => Some(Layer::Effect),
        _ => None,
    }
}

pub fn check(workspace: &Workspace) -> Outcome {
    let mut problems = Vec::new();
    for package in &workspace.packages {
        if UNCHECKED.contains(&package.name.as_str()) {
            continue;
        }
        let Some(own) = layer(&package.name) else {
            problems.push(format!(
                "{}: крейт не отнесён ни к одному слою.\n  → Добавь его в xtask/src/deps.rs и в карту крейтов docs/ARCHITECTURE.md.",
                package.name
            ));
            continue;
        };
        for dep in &package.deps {
            if let Err(reason) = allowed(&package.name, own, dep) {
                problems.push(format!("{} → {dep}: {reason}", package.name));
            }
        }
    }
    if problems.is_empty() {
        Outcome::Pass(String::new())
    } else {
        Outcome::Fail(format!(
            "нарушены правила зависимостей:\n{}\n→ Перенеси общий код в крейт ниже по слоям (часто в dc-core) или передавай данные через параметры.",
            problems.join("\n")
        ))
    }
}

fn allowed(name: &str, own: Layer, dep: &str) -> Result<(), String> {
    let Some(theirs) = layer(dep) else {
        return Err("зависимость от крейта вне слоёв".to_owned());
    };
    if own == Layer::Effect {
        let annotation_base = ANNOTATION_KINDS.contains(&name) && dep == "dc-fx-annotations";
        if EFFECT_MAY_USE.contains(&dep) || annotation_base {
            return Ok(());
        }
        return Err(format!(
            "эффект может зависеть только от {} (виды аннотаций ещё от dc-fx-annotations)",
            EFFECT_MAY_USE.join(", ")
        ));
    }
    let same_base = own == Layer::Base && theirs == Layer::Base;
    if theirs < own || same_base {
        Ok(())
    } else {
        Err(format!("слой {own:?} не может зависеть от слоя {theirs:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Package;

    fn check_one(name: &str, deps: &[&str]) -> Outcome {
        let package = Package {
            name: name.to_owned(),
            dir: String::new(),
            deps: deps.iter().map(|d| (*d).to_owned()).collect(),
        };
        check(&Workspace {
            root: "/ws".into(),
            packages: vec![package],
        })
    }

    fn passes(name: &str, deps: &[&str]) -> bool {
        matches!(check_one(name, deps), Outcome::Pass(_))
    }

    #[test]
    fn core_depends_on_nothing() {
        assert!(passes("dc-core", &[]));
        assert!(!passes("dc-core", &["dc-render"]));
    }

    #[test]
    fn base_crates_may_use_each_other() {
        assert!(passes("dc-encode", &["dc-core", "dc-media", "dc-render"]));
        assert!(!passes("dc-render", &["dc-fx-zoom"]));
    }

    #[test]
    fn effects_are_isolated() {
        assert!(passes("dc-fx-zoom", &["dc-core", "dc-render", "dc-ui-kit"]));
        assert!(!passes("dc-fx-zoom", &["dc-fx-cursor"]));
        assert!(!passes("dc-fx-zoom", &["dc-media"]));
    }

    #[test]
    fn annotation_kinds_may_use_annotation_base() {
        assert!(passes("dc-fx-text", &["dc-fx-annotations"]));
        assert!(!passes("dc-fx-zoom", &["dc-fx-annotations"]));
    }

    #[test]
    fn nobody_depends_on_top() {
        assert!(passes("dc-cli", &["dc-ops", "dc-core"]));
        assert!(!passes("dc-ops", &["dc-cli"]));
        assert!(!passes("dc-app", &["dc-cli"]));
    }

    #[test]
    fn unknown_crate_is_reported() {
        assert!(!passes("dc-new-thing", &[]));
    }
}
