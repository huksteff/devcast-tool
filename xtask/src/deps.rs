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
const BASE: [&str; 12] = [
    "dc-project",
    "dc-media",
    "dc-audio",
    "dc-render",
    "dc-encode",
    "dc-models",
    "dc-settings",
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

/// A crate that must not be built into another one, directly or through any chain of
/// non-dev dependencies, and why.
struct NeverLinks {
    package: &'static str,
    forbidden: &'static str,
    why: &'static str,
}

const NEVER_LINKS: [NeverLinks; 1] = [NeverLinks {
    package: "dc-app",
    forbidden: "dc-transcribe",
    why: "окно программы не тянет whisper и Vulkan (решение 018): распознаёт `devcast --captions-worker`, модели качает dc-models",
}];

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
    problems.extend(never_linked(workspace));
    if problems.is_empty() {
        Outcome::Pass(String::new())
    } else {
        Outcome::Fail(format!(
            "нарушены правила зависимостей:\n{}\n→ Перенеси общий код в крейт ниже по слоям (часто в dc-core) или передавай данные через параметры.",
            problems.join("\n")
        ))
    }
}

/// The [`NEVER_LINKS`] rules broken: the chain through which the forbidden crate gets in.
fn never_linked(workspace: &Workspace) -> Vec<String> {
    let links = |name: &str| -> &[String] {
        workspace
            .packages
            .iter()
            .find(|p| p.name == name)
            .map_or(&[], |p| p.links.as_slice())
    };
    let mut problems = Vec::new();
    for rule in &NEVER_LINKS {
        // Breadth-first walk keeping the chain to each crate reached.
        let mut chains: Vec<Vec<&str>> = vec![vec![rule.package]];
        let mut seen = vec![rule.package];
        let mut next = 0;
        while let Some(chain) = chains.get(next).cloned() {
            next += 1;
            let last = chain.last().copied().unwrap_or(rule.package);
            if last == rule.forbidden {
                problems.push(format!("{}: {}", chain.join(" → "), rule.why));
                break;
            }
            for dep in links(last) {
                if !seen.contains(&dep.as_str()) {
                    seen.push(dep);
                    let mut longer = chain.clone();
                    longer.push(dep);
                    chains.push(longer);
                }
            }
        }
    }
    problems
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
            links: deps.iter().map(|d| (*d).to_owned()).collect(),
        };
        check(&Workspace {
            root: "/ws".into(),
            target: "/ws/target".into(),
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

    #[test]
    fn the_app_never_links_whisper() {
        let package = |name: &str, links: &[&str], dev: &[&str]| Package {
            name: name.to_owned(),
            dir: String::new(),
            deps: links.iter().chain(dev).map(|d| (*d).to_owned()).collect(),
            links: links.iter().map(|d| (*d).to_owned()).collect(),
        };
        let workspace = |ops_links: &[&str], app_dev: &[&str]| Workspace {
            root: "/ws".into(),
            target: "/ws/target".into(),
            packages: vec![
                package("dc-core", &[], &[]),
                package("dc-models", &[], &[]),
                package("dc-transcribe", &["dc-models"], &[]),
                package("dc-ops", ops_links, &[]),
                package("dc-app", &["dc-ops", "dc-models"], app_dev),
            ],
        };
        assert!(never_linked(&workspace(&["dc-core"], &[])).is_empty());
        let through_ops = never_linked(&workspace(&["dc-core", "dc-transcribe"], &[]));
        assert_eq!(through_ops.len(), 1);
        assert!(
            through_ops[0].starts_with("dc-app → dc-ops → dc-transcribe:"),
            "{through_ops:?}"
        );
        assert!(
            never_linked(&workspace(&["dc-core"], &["dc-transcribe"])).is_empty(),
            "dev dependencies are not built into the exe"
        );
    }
}
