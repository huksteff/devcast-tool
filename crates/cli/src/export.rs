//! `devcast export -o урок.mp4`: the export joins the queue (`export_queue`) and the call waits
//! for its file; `export status` shows the queue, `export cancel [e2]` cancels.

use dc_capture::win::on_console_stop;
use dc_encode::export::{GIF_FPS, MP4_FPS};
use dc_ops::export::{Format, Spec};
use dc_render::{AdapterChoice, Gpu};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::export_args::{ExportArgs, settings};
use crate::export_queue::{self, Finished, Job, Reply, Request, Server};
use crate::find::find_project;
use crate::run::Answer;
use crate::workspace::Workspace;

/// `export status` and `export cancel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Action {
    Status,
    Cancel,
}

/// The real channel of the queue; tests use their own.
pub(crate) fn channel() -> Result<String, CliError> {
    dc_ipc::socket_name("export").map_err(|e| CliError::Failed(e.to_string()))
}

pub(crate) fn export(
    ctx: &Ctx<'_>,
    action: Option<Action>,
    id: Option<&str>,
    args: &ExportArgs,
    channel: &str,
) -> Result<Answer, CliError> {
    match (action, id) {
        (None, Some(_)) => Err(CliError::Usage(
            "лишний аргумент: devcast export -o урок.mp4, export status или export cancel e2"
                .to_owned(),
        )),
        (Some(_), _) if !args.is_empty() => Err(CliError::Usage(
            "export status и export cancel не берут настроек экспорта".to_owned(),
        )),
        (Some(Action::Status), Some(_)) => Err(CliError::Usage(
            "export status показывает всю очередь: devcast export status".to_owned(),
        )),
        (Some(Action::Status), None) => status(channel),
        (Some(Action::Cancel), id) => cancel(channel, id.map(parse_id).transpose()?),
        (None, None) => start(ctx, args, channel),
    }
}

fn start(ctx: &Ctx<'_>, args: &ExportArgs, channel: &str) -> Result<Answer, CliError> {
    let (out, settings) = settings(args, ctx.cwd)?;
    let allowed: &[u32] = match settings.format {
        Format::Mp4 => &MP4_FPS,
        Format::Gif => &GIF_FPS,
    };
    if !allowed.contains(&settings.fps) {
        let list: Vec<_> = allowed.iter().map(u32::to_string).collect();
        return Err(CliError::Usage(format!(
            "--fps {}: для {} можно {}",
            settings.fps,
            if settings.format == Format::Mp4 {
                "mp4"
            } else {
                "gif"
            },
            list.join(", ")
        )));
    }
    let dir = find_project(ctx.globals.project.as_deref(), ctx.cwd)?;
    let workspace = Workspace::open(dir, false, ctx.globals.dry_run)?;
    let spec = Spec {
        root: workspace.dir().root().to_path_buf(),
        project: workspace.project.clone(),
        settings,
        out,
    };
    if ctx.globals.dry_run {
        let text = format!("dry-run, экспорт не начат: {}", spec.label());
        let json = json!({"dry_run": true, "out": spec.out, "settings": spec.settings});
        return Ok(Answer::new(text, json));
    }
    let mut answer = run(spec, channel)?;
    answer.warnings.extend(workspace.warnings);
    Ok(answer)
}

/// Sends `spec` to the running queue or runs the queue here.
fn run(spec: Spec, channel: &str) -> Result<Answer, CliError> {
    let request = || Request::Run(Box::new(spec.clone()));
    if let Some(reply) = export_queue::ask(channel, &request())? {
        return done(reply);
    }
    let gpu = Gpu::new(AdapterChoice::Auto).map_err(|e| CliError::Failed(e.to_string()))?;
    match Server::start(channel, gpu)? {
        Some(server) => serve(server, spec),
        // Another call took the channel between the two steps: its queue takes this export.
        None => match export_queue::ask(channel, &request())? {
            Some(reply) => done(reply),
            None => Err(CliError::Failed(
                "очередь экспорта только что закрылась: начни экспорт ещё раз".to_owned(),
            )),
        },
    }
}

/// Runs the queue in this process: this export first, then those sent by other calls until
/// the queue is empty.
fn serve(server: Server, spec: Spec) -> Result<Answer, CliError> {
    let id = server.push(spec);
    let queue = server.queue.clone();
    // Only one handler per process; a failure leaves Ctrl+C as it is and the export runs on.
    let ctrl_c = on_console_stop(move || {
        queue.cancel(None);
    })
    .err()
    .map(|e| format!("Ctrl+C не отменит экспорт ({e}): отмена — devcast export cancel"));
    let state = server.queue.wait(id).ok_or_else(|| {
        CliError::Failed(format!(
            "экспорт e{id} пропал из очереди. Это ошибка программы: сообщи разработчику"
        ))
    })?;
    let warnings = server.close()?;
    let finished = export_queue::finished(id, state);
    let mut answer = done(Reply::Finished(finished))?;
    answer.warnings.extend(ctrl_c);
    answer.warnings.extend(warnings);
    Ok(answer)
}

fn done(reply: Reply) -> Result<Answer, CliError> {
    let Reply::Finished(finished) = reply else {
        return Err(CliError::Failed(
            "очередь экспорта ответила не тем: обнови devcast".to_owned(),
        ));
    };
    let json = serde_json::to_value(&finished)
        .map_err(|e| CliError::Failed(format!("ответ экспорта не записался в JSON: {e}")))?;
    match finished {
        Finished::Done {
            id,
            path,
            width,
            height,
            fps,
            duration,
            bytes,
            codec,
            seconds,
            ..
        } => {
            let codec = codec.map(|c| format!(", {c}")).unwrap_or_default();
            let size = megabytes(bytes);
            let speed = duration / seconds;
            let text = format!(
                "e{id} готово: {path} — {width}×{height}, {fps} fps, {duration:.1} с, {size}{codec}; за {seconds:.1} с (×{speed:.1} реального времени)"
            );
            Ok(Answer::new(text, json))
        }
        Finished::Failed { id, message } => Err(CliError::Failed(format!(
            "e{id}: экспорт не удался: {message}"
        ))),
        Finished::Cancelled { id } => Err(CliError::Failed(format!(
            "e{id}: экспорт отменён, файл не записан"
        ))),
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} МБ", bytes as f64 / 1_048_576.0)
}

fn status(channel: &str) -> Result<Answer, CliError> {
    let jobs = match export_queue::ask(channel, &Request::Status)? {
        None => Vec::new(),
        Some(Reply::Status { jobs }) => jobs,
        Some(_) => {
            return Err(CliError::Failed(
                "очередь экспорта ответила не тем: обнови devcast".to_owned(),
            ));
        }
    };
    let text = match jobs.is_empty() {
        true => "экспорт не идёт".to_owned(),
        false => jobs.iter().map(line).collect::<Vec<_>>().join("\n"),
    };
    Ok(Answer::new(text, json!({ "jobs": jobs })))
}

fn line(job: &Job) -> String {
    let state = match job.state.as_str() {
        "running" if job.total > 0 && job.done > 0 => {
            let left = job.elapsed * (job.total - job.done) as f64 / job.done as f64;
            format!(
                "идёт {}% ({}/{} кадров), осталось ~{left:.0} с",
                job.done * 100 / job.total,
                job.done,
                job.total
            )
        }
        "running" => "идёт".to_owned(),
        "queued" => "в очереди".to_owned(),
        "done" => format!("готово за {:.1} с", job.elapsed),
        "cancelled" => "отменён".to_owned(),
        _ => format!("ошибка: {}", job.error.as_deref().unwrap_or("")),
    };
    format!("e{}  {state}  {}", job.id, job.label)
}

fn cancel(channel: &str, id: Option<u32>) -> Result<Answer, CliError> {
    let ids = match export_queue::ask(channel, &Request::Cancel { id })? {
        Some(Reply::Cancelled { ids }) => ids,
        None => Vec::new(),
        Some(_) => {
            return Err(CliError::Failed(
                "очередь экспорта ответила не тем: обнови devcast".to_owned(),
            ));
        }
    };
    if ids.is_empty() {
        return Err(CliError::Failed(match id {
            Some(id) => {
                format!("e{id} не идёт и не ждёт в очереди: список — devcast export status")
            }
            None => "экспорт не идёт: отменять нечего".to_owned(),
        }));
    }
    let list: Vec<_> = ids.iter().map(|id| format!("e{id}")).collect();
    Ok(Answer::new(
        format!("отменено: {}", list.join(", ")),
        json!({ "cancelled": ids }),
    ))
}

fn parse_id(text: &str) -> Result<u32, CliError> {
    text.strip_prefix('e')
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| {
            CliError::Usage(format!(
                "«{text}» — не номер экспорта: e1, e2 … (devcast export status)"
            ))
        })
}
