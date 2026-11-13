//! `devcast export -o урок.mp4`: the export joins the queue (`export_queue`) and the call waits
//! for its file; `export status` shows the queue, `export cancel [e2]` cancels.

use dc_capture::win::on_console_stop;
use dc_core::{Command, OutputTime, Project, TimeMap};
use dc_encode::export::{GIF_FPS, MP4_FPS};
use dc_fx_cut::Cut;
use dc_ops::export::{Format, Spec};
use dc_render::{AdapterChoice, Gpu};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::export_app::{self, Queue, parse_id};
use crate::export_args::{ExportArgs, settings};
use crate::export_queue::{self, Finished, Job, Reply, Request, Server};
use crate::find::find_project;
use crate::live::app_channel;
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
        (Some(Action::Status), None) => status(ctx, channel),
        (Some(Action::Cancel), id) => cancel(ctx, channel, id.map(parse_id).transpose()?),
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
    let app = ctx.local.app_channel;
    let workspace = Workspace::open(dir, false, ctx.globals.dry_run, app)?;
    let spec = Spec {
        root: workspace.dir().root().to_path_buf(),
        project: piece(&workspace.project, args)?,
        settings,
        out,
    };
    if ctx.globals.dry_run {
        let text = format!("dry-run, экспорт не начат: {}", spec.label());
        let json = json!({"dry_run": true, "out": spec.out, "settings": spec.settings});
        return Ok(Answer::new(text, json));
    }
    let app = workspace.editor().map(|e| e.channel().to_owned());
    let in_app = match app {
        Some(app) => export_app::run(&app, &spec)?,
        None => None,
    };
    let mut answer = match in_app {
        Some(finished) => done(finished, Queue::App)?,
        None => run(spec, channel)?,
    };
    answer.warnings.extend(workspace.warnings);
    Ok(answer)
}

/// `--from`, `--to`: `project` as it is exported — the rest of the video cut away (`cut`, in
/// memory only: the project on disk stays whole).
fn piece(project: &Project, args: &ExportArgs) -> Result<Project, CliError> {
    let (from, to) = (
        crate::ls::time(args.from.as_ref(), "--from")?,
        crate::ls::time(args.to.as_ref(), "--to")?,
    );
    if from.is_none() && to.is_none() {
        return Ok(project.clone());
    }
    let end = OutputTime::ZERO + TimeMap::build(project).duration();
    let (start, stop) = (from.unwrap_or(OutputTime::ZERO), to.unwrap_or(end));
    if start >= stop || stop > end {
        return Err(CliError::Usage(format!(
            "--from {start} --to {stop}: кусок — внутри видео (0:00–{end}), начало раньше конца"
        )));
    }
    let outside: Vec<String> = [(OutputTime::ZERO, start), (stop, end)]
        .into_iter()
        .filter(|(a, b)| a < b)
        .map(|(a, b)| format!("{}-{}", a.as_secs_f64(), b.as_secs_f64()))
        .collect();
    let mut piece = project.clone();
    if !outside.is_empty() {
        let failed = |e: &dyn std::fmt::Display| CliError::Failed(e.to_string());
        let cut: Cut =
            serde_json::from_value(json!({"ranges": outside.join(",")})).map_err(|e| failed(&e))?;
        cut.apply(&mut piece).map_err(|e| failed(&e))?;
    }
    Ok(piece)
}

/// Sends `spec` to the running queue or runs the queue here.
fn run(spec: Spec, channel: &str) -> Result<Answer, CliError> {
    let request = || Request::Run(Box::new(spec.clone()));
    if let Some(reply) = export_queue::ask(channel, &request())? {
        return finished(reply);
    }
    let gpu = Gpu::new(AdapterChoice::Auto).map_err(|e| CliError::Failed(e.to_string()))?;
    match Server::start(channel, gpu)? {
        Some(server) => serve(server, spec),
        // Another call took the channel between the two steps: its queue takes this export.
        None => match export_queue::ask(channel, &request())? {
            Some(reply) => finished(reply),
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
    let mut answer = done(export_queue::finished(id, state), Queue::Cli)?;
    answer.warnings.extend(ctrl_c);
    answer.warnings.extend(warnings);
    Ok(answer)
}

fn finished(reply: Reply) -> Result<Answer, CliError> {
    match reply {
        Reply::Finished(finished) => done(finished, Queue::Cli),
        _ => Err(CliError::Failed(
            "очередь экспорта ответила не тем: обнови devcast".to_owned(),
        )),
    }
}

/// How the export of `queue` ended.
fn done(finished: Finished, queue: Queue) -> Result<Answer, CliError> {
    let mut json = serde_json::to_value(&finished)
        .map_err(|e| CliError::Failed(format!("ответ экспорта не записался в JSON: {e}")))?;
    json["queue"] = queue.name().into();
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
                "{} готово: {path} — {width}×{height}, {fps} fps, {duration:.1} с, {size}{codec}; за {seconds:.1} с (×{speed:.1} реального времени)",
                queue.tag(id)
            );
            Ok(Answer::new(text, json))
        }
        Finished::Failed { id, message } => Err(CliError::Failed(format!(
            "{}: экспорт не удался: {message}",
            queue.tag(id)
        ))),
        Finished::Cancelled { id } => Err(CliError::Failed(format!(
            "{}: экспорт отменён, файл не записан",
            queue.tag(id)
        ))),
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} МБ", bytes as f64 / 1_048_576.0)
}

/// The channel of the queue of `devcast export` processes in this run.
pub(crate) fn run_channel(ctx: &Ctx<'_>) -> Result<String, CliError> {
    match ctx.local.export_channel {
        Some(channel) => Ok(channel.to_owned()),
        None => channel(),
    }
}

/// The jobs of both queues: of `devcast export` processes and the app's.
pub(crate) fn all_jobs(ctx: &Ctx<'_>) -> Result<Vec<(Queue, Job)>, CliError> {
    jobs(ctx, &run_channel(ctx)?)
}

fn jobs(ctx: &Ctx<'_>, channel: &str) -> Result<Vec<(Queue, Job)>, CliError> {
    let cli = match export_queue::ask(channel, &Request::Status)? {
        None => Vec::new(),
        Some(Reply::Status { jobs }) => jobs,
        Some(_) => {
            return Err(CliError::Failed(
                "очередь экспорта ответила не тем: обнови devcast".to_owned(),
            ));
        }
    };
    let app = export_app::jobs(&app_channel(ctx.local.app_channel)?)?;
    let cli = cli.into_iter().map(|j| (Queue::Cli, j));
    Ok(cli
        .chain(app.into_iter().map(|j| (Queue::App, j)))
        .collect())
}

/// Both queues.
fn status(ctx: &Ctx<'_>, channel: &str) -> Result<Answer, CliError> {
    let jobs = jobs(ctx, channel)?;
    let text = match jobs.is_empty() {
        true => "экспорт не идёт".to_owned(),
        false => jobs.iter().map(line).collect::<Vec<_>>().join("\n"),
    };
    let listed: Vec<_> = jobs
        .iter()
        .map(|(queue, job)| listed(*queue, job))
        .collect();
    Ok(Answer::new(text, json!({ "jobs": listed })))
}

/// A job in JSON: its fields, its queue and its tag (`e2`, `p2`) for `export cancel`.
pub(crate) fn listed(queue: Queue, job: &Job) -> serde_json::Value {
    let mut value = json!(job);
    value["queue"] = queue.name().into();
    value["tag"] = queue.tag(job.id).into();
    value
}

pub(crate) fn line((queue, job): &(Queue, Job)) -> String {
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
    let place = match queue {
        Queue::Cli => "",
        Queue::App => "  (программа)",
    };
    format!("{}  {state}  {}{place}", queue.tag(job.id), job.label)
}

/// One job of either queue, or every job of both.
fn cancel(ctx: &Ctx<'_>, channel: &str, id: Option<(Queue, u32)>) -> Result<Answer, CliError> {
    let mut cancelled = Vec::new();
    if id.is_none_or(|(queue, _)| queue == Queue::Cli) {
        let one = id.map(|(_, id)| id);
        let ids = match export_queue::ask(channel, &Request::Cancel { id: one })? {
            Some(Reply::Cancelled { ids }) => ids,
            None => Vec::new(),
            Some(_) => {
                return Err(CliError::Failed(
                    "очередь экспорта ответила не тем: обнови devcast".to_owned(),
                ));
            }
        };
        cancelled.extend(ids.into_iter().map(|id| Queue::Cli.tag(id)));
    }
    if id.is_none_or(|(queue, _)| queue == Queue::App) {
        let app = app_channel(ctx.local.app_channel)?;
        let ids = export_app::cancel(&app, id.map(|(_, id)| id))?;
        cancelled.extend(ids.into_iter().map(|id| Queue::App.tag(id)));
    }
    if cancelled.is_empty() {
        return Err(CliError::Failed(match id {
            Some((queue, id)) => format!(
                "{} не идёт и не ждёт в очереди: список — devcast export status",
                queue.tag(id)
            ),
            None => "экспорт не идёт: отменять нечего".to_owned(),
        }));
    }
    Ok(Answer::new(
        format!("отменено: {}", cancelled.join(", ")),
        json!({ "cancelled": cancelled }),
    ))
}
