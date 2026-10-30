//! Every field and variant of a `derive(JsonSchema)` type has a `///` doc comment:
//! the docs feed `devcast help` and the JSON Schema (CLAUDE.md, «Правила кода»).
//! Test types are checked too: one rule for every schema type. Fields and variants with
//! `serde(skip)` or `schemars(skip)` are not in the schema and need no doc.

use std::path::Path;

use syn::punctuated::Punctuated;
use syn::{Attribute, Fields, Item, Meta, Token};

use crate::run::Outcome;
use crate::sources;

pub fn check(root: &Path) -> Outcome {
    let files = match sources::rust_files(root) {
        Ok(files) => files,
        Err(error) => return Outcome::Fail(error),
    };
    let mut problems = Vec::new();
    for path in &files {
        let shown = sources::display(root, path);
        match sources::read(path) {
            Ok(text) => match undocumented(&shown, &text) {
                Ok(found) => problems.extend(found),
                Err(error) => problems.push(error),
            },
            Err(error) => problems.push(error),
        }
    }
    if problems.is_empty() {
        Outcome::Pass(String::new())
    } else {
        Outcome::Fail(format!(
            "{}\n→ Добавь `///` над каждым полем и вариантом: что это, единицы, диапазон (по-английски). \
             Из них собираются `devcast help` и JSON Schema.",
            problems.join("\n")
        ))
    }
}

fn undocumented(shown: &str, text: &str) -> Result<Vec<String>, String> {
    let file = syn::parse_file(text)
        .map_err(|error| format!("{shown}: не удалось разобрать файл: {error}"))?;
    let mut found = Vec::new();
    visit(&file.items, shown, &mut found);
    Ok(found)
}

fn visit(items: &[Item], shown: &str, found: &mut Vec<String>) {
    for item in items {
        match item {
            Item::Struct(item) if derives_schema(&item.attrs) => {
                fields(&item.fields, &item.ident.to_string(), shown, found);
            }
            Item::Enum(item) if derives_schema(&item.attrs) => {
                for variant in &item.variants {
                    let name = format!("{}::{}", item.ident, variant.ident);
                    if !has_doc(&variant.attrs) && !skipped(&variant.attrs) {
                        found.push(report(shown, variant.ident.span(), &name));
                    }
                    fields(&variant.fields, &name, shown, found);
                }
            }
            Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    visit(inner, shown, found);
                }
            }
            _ => {}
        }
    }
}

fn fields(fields: &Fields, owner: &str, shown: &str, found: &mut Vec<String>) {
    for field in fields {
        if let Some(ident) = &field.ident
            && !has_doc(&field.attrs)
            && !skipped(&field.attrs)
        {
            found.push(report(shown, ident.span(), &format!("{owner}::{ident}")));
        }
    }
}

fn report(shown: &str, span: proc_macro2::Span, name: &str) -> String {
    format!("{shown}:{}: `{name}` без `///`", span.start().line)
}

fn derives_schema(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("derive"))
        .any(|a| {
            let mut found = false;
            let parsed = a.parse_nested_meta(|meta| {
                found |= meta
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "JsonSchema");
                Ok(())
            });
            parsed.is_ok() && found
        })
}

fn has_doc(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("doc"))
}

fn skipped(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("serde") || a.path().is_ident("schemars"))
        .filter_map(|a| {
            a.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()
        })
        .flatten()
        .any(|meta| meta.path().is_ident("skip"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_text(text: &str) -> Vec<String> {
        undocumented("x.rs", text).unwrap()
    }

    #[test]
    fn reports_undocumented_field_with_line() {
        let text = "#[derive(Debug, JsonSchema)]\nstruct A {\n    /// Width, px.\n    w: u32,\n    h: u32,\n}\n";
        assert_eq!(check_text(text), ["x.rs:5: `A::h` без `///`"]);
    }

    #[test]
    fn reports_variant_and_its_fields() {
        let text = "#[derive(schemars::JsonSchema)]\nenum E {\n    /// Ok.\n    A,\n    B { x: f32 },\n}\n";
        assert_eq!(
            check_text(text),
            ["x.rs:5: `E::B` без `///`", "x.rs:5: `E::B::x` без `///`"]
        );
    }

    #[test]
    fn ignores_types_without_schema_and_tuple_fields() {
        let text = "#[derive(Debug)]\nstruct A { w: u32 }\n#[derive(JsonSchema)]\nstruct B(u32);\n";
        assert!(check_text(text).is_empty());
    }

    #[test]
    fn skipped_fields_need_no_doc() {
        let text = "#[derive(JsonSchema)]
struct A {
    #[serde(skip)]
    a: u32,
    #[schemars(skip)]
    b: u32,
    #[serde(default, skip_serializing_if = \"x\")]
    c: u32,
}
";
        assert_eq!(check_text(text), ["x.rs:8: `A::c` без `///`"]);
    }

    #[test]
    fn looks_inside_inline_modules() {
        let text = "mod m {\n    #[derive(JsonSchema)]\n    struct A { w: u32 }\n}\n";
        assert_eq!(check_text(text), ["x.rs:3: `A::w` без `///`"]);
    }

    #[test]
    fn unparsable_file_is_an_error() {
        assert!(undocumented("x.rs", "fn (").is_err());
    }
}
