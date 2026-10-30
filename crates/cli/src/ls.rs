//! `devcast info` and `devcast ls <list>`: what is in the project, in output time.

use dc_core::time::Output;
use dc_core::{OutputTime, Place, Project, Range, Rect, TimeMap};
use dc_ops::Registry;
use serde_json::{Map, Value, json};

use crate::edit::{Ctx, answer};
use crate::error::CliError;
use crate::run::Answer;

const CLIPS: &str = "clips";

/// Filters and paging of `devcast ls`.
pub(crate) struct Query {
    pub list: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub fields: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

/// One object of a list: where it plays (`None` if cut out) and its other fields.
struct Row {
    id: String,
    span: Option<Range<Output>>,
    fields: Map<String, Value>,
}

fn lists(registry: &Registry) -> Vec<&'static str> {
    let effects = registry
        .effects()
        .iter()
        .flat_map(|e| e.lists.iter().copied());
    std::iter::once(CLIPS).chain(effects).collect()
}

fn rows(list: &str, project: &Project, registry: &Registry) -> Result<Vec<Row>, CliError> {
    let map = TimeMap::build(project);
    let mut rows = Vec::new();
    if list == CLIPS {
        for clip in &project.clips {
            let mut fields = Map::new();
            fields.insert("media".into(), clip.media.path.clone().into());
            fields.insert("kind".into(), json!(clip.media.kind));
            let trim = format!("{}–{}", clip.trim.start(), clip.trim.end());
            fields.insert("trim".into(), trim.into());
            fields.insert("cuts".into(), clip.cuts.len().into());
            fields.insert("speeds".into(), clip.speeds.len().into());
            if clip.crop != Rect::FULL {
                let c = clip.crop;
                let crop = format!("{},{},{},{}", c.x(), c.y(), c.w(), c.h());
                fields.insert("crop".into(), crop.into());
            }
            rows.push(Row {
                id: clip.id.to_string(),
                span: clip_span(&map, clip.id),
                fields,
            });
        }
        return Ok(rows);
    }
    let effects = registry
        .effects()
        .iter()
        .filter(|e| e.lists.contains(&list));
    for effect in effects {
        for item in (effect.items)(project).map_err(|e| CliError::Failed(e.to_string()))? {
            if item.list == list {
                let span = match &item.place {
                    Place::Source { media, range } => map.output_span(media, *range),
                    Place::Output(range) => Some(*range),
                };
                rows.push(Row {
                    span,
                    id: item.id,
                    fields: item.fields,
                });
            }
        }
    }
    rows.sort_by_key(|r| {
        r.span
            .map(|s| s.start())
            .unwrap_or(OutputTime::from_micros(i64::MAX))
    });
    Ok(rows)
}

/// Output range of one clip: from its first segment to its last.
fn clip_span(map: &TimeMap, clip: dc_core::ClipId) -> Option<Range<Output>> {
    let mut segments = map.segments().iter().filter(|s| s.clip == clip);
    let first = segments.next()?;
    let last = segments.next_back().unwrap_or(first);
    Range::new(first.output_start, last.output_end()).ok()
}

pub(crate) fn time(text: Option<&String>, flag: &str) -> Result<Option<OutputTime>, CliError> {
    text.map(|t| {
        t.parse::<OutputTime>()
            .map_err(|e| CliError::Usage(format!("{flag}: {e}")))
    })
    .transpose()
}

pub(crate) fn ls(ctx: &Ctx<'_>, query: &Query) -> Result<Answer, CliError> {
    let known = lists(ctx.registry);
    if !known.contains(&query.list.as_str()) {
        return Err(CliError::Usage(format!(
            "нет списка «{}»; есть: {}, sources",
            query.list,
            known.join(", ")
        )));
    }
    let mut workspace = ctx.open(false)?;
    let mut rows = rows(&query.list, &workspace.project, ctx.registry)?;
    let (from, to) = (
        time(query.from.as_ref(), "--from")?,
        time(query.to.as_ref(), "--to")?,
    );
    if from.is_some() || to.is_some() {
        let from = from.unwrap_or(OutputTime::ZERO);
        let to = to.unwrap_or(OutputTime::from_micros(i64::MAX));
        rows.retain(|r| r.span.is_some_and(|s| s.start() < to && from < s.end()));
    }
    if let Some(fields) = &query.fields {
        let wanted: Vec<&str> = fields.split(',').map(str::trim).collect();
        let present: Vec<&String> = rows.iter().flat_map(|r| r.fields.keys()).collect();
        let missing = wanted.iter().find(|w| !present.iter().any(|p| p == w));
        if let Some(missing) = missing.filter(|_| !rows.is_empty()) {
            let mut names: Vec<&str> = present.iter().map(|p| p.as_str()).collect();
            names.sort_unstable();
            names.dedup();
            return Err(CliError::Usage(format!(
                "нет поля «{missing}»; есть: {}",
                names.join(", ")
            )));
        }
        for row in &mut rows {
            row.fields.retain(|k, _| wanted.contains(&k.as_str()));
        }
    }
    let total = rows.len();
    let page: Vec<&Row> = rows.iter().skip(query.offset).take(query.limit).collect();
    let text = page_text(query, &page, total);
    let json = json!({
        "list": query.list,
        "total": total,
        "offset": query.offset,
        "items": page.iter().map(|r| row_json(r)).collect::<Vec<_>>(),
    });
    Ok(answer(&mut workspace, text, json))
}

fn page_text(query: &Query, page: &[&Row], total: usize) -> String {
    if page.is_empty() {
        return format!("{}: пусто (всего {total})", query.list);
    }
    let mut lines: Vec<String> = page.iter().map(|r| row_text(r)).collect();
    let shown = query.offset + page.len();
    if shown < total {
        lines.push(format!(
            "показано {}–{shown} из {total}; дальше: devcast ls {} --offset {shown}",
            query.offset + 1,
            query.list
        ));
    }
    lines.join("\n")
}

fn row_text(row: &Row) -> String {
    let span = match row.span {
        Some(s) => format!("{}–{}", s.start(), s.end()),
        None => "вырезан".to_owned(),
    };
    let mut line = format!("{}  {span}", row.id);
    for (key, value) in &row.fields {
        push_field(&mut line, key, value);
    }
    line
}

/// `key=value`; an object becomes one `key.inner=value` per field, so no JSON in plain text.
fn push_field(line: &mut String, key: &str, value: &Value) {
    match value {
        Value::String(text) => line.push_str(&format!("  {key}={text}")),
        Value::Object(fields) => {
            for (inner, value) in fields {
                push_field(line, &format!("{key}.{inner}"), value);
            }
        }
        other => line.push_str(&format!("  {key}={other}")),
    }
}

fn row_json(row: &Row) -> Value {
    let mut value = Map::new();
    value.insert("id".into(), row.id.clone().into());
    let secs = |t: OutputTime| Value::from(t.as_secs_f64());
    value.insert(
        "from".into(),
        row.span.map_or(Value::Null, |s| secs(s.start())),
    );
    value.insert("to".into(), row.span.map_or(Value::Null, |s| secs(s.end())));
    value.extend(row.fields.clone());
    Value::Object(value)
}

/// `devcast info`: name, length, unsaved state and how many objects each list has.
pub(crate) fn info(ctx: &Ctx<'_>) -> Result<Answer, CliError> {
    let mut workspace = ctx.open(false)?;
    let project = &workspace.project;
    let duration = OutputTime::ZERO + TimeMap::build(project).duration();
    let mut counts = Map::new();
    for list in lists(ctx.registry) {
        counts.insert(list.into(), rows(list, project, ctx.registry)?.len().into());
    }
    let name = workspace.name();
    let unsaved = workspace.unsaved();
    let counted: Vec<String> = counts.iter().map(|(k, v)| format!("{k} {v}")).collect();
    let text = format!(
        "{name}: длительность {duration}, {}\nсписки: {}; подробно: devcast ls <список>",
        match unsaved {
            true => "есть несохранённые изменения (devcast save)",
            false => "всё сохранено",
        },
        counted.join(", ")
    );
    let json = json!({
        "name": name,
        "duration": duration.as_secs_f64(),
        "unsaved": unsaved,
        "lists": counts,
    });
    Ok(answer(&mut workspace, text, json))
}
