//! `devcast frame` and `devcast sheet`: frames of the finished video as PNG for checking by eye.
//! Scenes come from `dc_ops::Evaluator` like the export's (`crate::scene`), PNGs from
//! `dc_encode::frame_png` / `sheet_png`. Times are output times.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use dc_core::{OutputTime, PixelSize, TimeDelta};
use dc_encode::{Sources, frame_png, sheet_png};
use dc_render::{AdapterChoice, Gpu, SHEET_GAP, Scene};
use serde_json::json;

use crate::edit::Ctx;
use crate::error::CliError;
use crate::run::Answer;
use crate::workspace::Workspace;

/// Most frames of a sheet: more would make each too small to read.
const SHEET_MAX: u32 = 36;
/// Narrowest frame of `--width`, px: smaller shows nothing.
const WIDTH_MIN: u32 = 16;

#[derive(Debug, clap::Args)]
pub(crate) struct FrameArgs {
    #[arg(long, help = "Момент готового видео: 12.5 или 1:02.5")]
    at: String,
    #[arg(
        long,
        help = "Сетка через 0.1 кадра с подписями: доли кадра, как у --rect, --x/--y и точек рисунка (без зума)"
    )]
    grid: bool,
    #[arg(
        long,
        help = "Ширина картинки, px: кадр уменьшается с теми же пропорциями; без неё — размер кадра проекта"
    )]
    width: Option<u32>,
    #[arg(short = 'o', long, help = "PNG-файл, куда писать")]
    out: PathBuf,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SheetArgs {
    #[arg(long, help = "Начало отрезка готового видео; без него — 0")]
    from: Option<String>,
    #[arg(long, help = "Конец отрезка; без него — конец видео")]
    to: Option<String>,
    #[arg(
        short = 'n',
        default_value_t = 6,
        help = "Сколько кадров, 1–36: начала n равных частей отрезка"
    )]
    count: u32,
    #[arg(
        long,
        help = "Ширина всей раскадровки, px: кадры уменьшаются с теми же пропорциями; без неё — размер кадра проекта"
    )]
    width: Option<u32>,
    #[arg(short = 'o', long, help = "PNG-файл, куда писать")]
    out: PathBuf,
}

pub(crate) fn frame(ctx: &Ctx<'_>, args: &FrameArgs) -> Result<Answer, CliError> {
    let out = png_path(ctx.cwd, &args.out)?;
    let at = crate::ls::time(Some(&args.at), "--at")?.unwrap_or(OutputTime::ZERO);
    let workspace = ctx.open(false)?;
    let evaluator = crate::scene::evaluator(workspace.dir().root(), &workspace.project)?;
    let end = OutputTime::ZERO + evaluator.duration();
    if at < OutputTime::ZERO || at >= end {
        return Err(CliError::Failed(format!(
            "--at {at}: такого момента нет, видео идёт от 0:00.000 до {end}"
        )));
    }
    let canvas = evaluator.canvas();
    let size = match args.width {
        Some(width) => scaled(canvas, width, "--width")?,
        None => canvas,
    };
    let text = format!("кадр {at} ({}×{})", size.width, size.height);
    let json = json!({"out": out, "at": at.as_secs_f64(), "size": [size.width, size.height]});
    let mut scene = evaluator.evaluate(at);
    if args.grid {
        crate::grid::draw(&mut scene);
    }
    write(
        ctx,
        &workspace,
        &out,
        text,
        json,
        |gpu, compositor, sources| frame_png(gpu, compositor, sources, &scene, size),
    )
}

pub(crate) fn sheet(ctx: &Ctx<'_>, args: &SheetArgs) -> Result<Answer, CliError> {
    let out = png_path(ctx.cwd, &args.out)?;
    if !(1..=SHEET_MAX).contains(&args.count) {
        return Err(CliError::Usage(format!(
            "-n {}: кадров можно от 1 до {SHEET_MAX}",
            args.count
        )));
    }
    let from = crate::ls::time(args.from.as_ref(), "--from")?.unwrap_or(OutputTime::ZERO);
    let to = crate::ls::time(args.to.as_ref(), "--to")?;
    let workspace = ctx.open(false)?;
    let evaluator = crate::scene::evaluator(workspace.dir().root(), &workspace.project)?;
    let end = OutputTime::ZERO + evaluator.duration();
    let to = to.unwrap_or(end).min(end);
    if from < OutputTime::ZERO || from >= to {
        return Err(CliError::Failed(format!(
            "отрезок {from}–{to} пуст: нужно 0 ≤ --from < --to, видео идёт до {end}"
        )));
    }
    let times = moments(from, to, args.count);
    let columns = columns(args.count);
    let rows = args.count.div_ceil(columns.get());
    let canvas = evaluator.canvas();
    let cell = match args.width {
        Some(width) => {
            let gaps = (columns.get() + 1) * SHEET_GAP;
            let what = format!("--width {width} на {columns} столбца с полями по {SHEET_GAP} px");
            scaled(canvas, width.saturating_sub(gaps) / columns.get(), &what)?
        }
        None => canvas,
    };
    let list: Vec<String> = times.iter().map(ToString::to_string).collect();
    let text = format!(
        "раскадровка {} кадров ({}×{rows}): {}",
        args.count,
        columns,
        list.join(", ")
    );
    let secs: Vec<f64> = times.iter().map(|t| t.as_secs_f64()).collect();
    let json = json!({"out": out, "at": secs, "columns": columns.get()});
    let scenes: Vec<Scene> = times.iter().map(|&t| evaluator.evaluate(t)).collect();
    write(
        ctx,
        &workspace,
        &out,
        text,
        json,
        |gpu, compositor, sources| sheet_png(gpu, compositor, sources, &scenes, columns, cell),
    )
}

/// `canvas` scaled to `width`, the same aspect; no wider than `canvas` and no narrower than
/// [`WIDTH_MIN`].
fn scaled(canvas: PixelSize, width: u32, what: &str) -> Result<PixelSize, CliError> {
    if !(WIDTH_MIN..=canvas.width).contains(&width) {
        return Err(CliError::Failed(format!(
            "{what}: ширина кадра выходит {width} px, а можно от {WIDTH_MIN} до {} (ширина кадра проекта)",
            canvas.width
        )));
    }
    Ok(PixelSize {
        width,
        height: canvas.height * width / canvas.width,
    })
}

/// Starts of `count` equal parts of `from..to`.
fn moments(from: OutputTime, to: OutputTime, count: u32) -> Vec<OutputTime> {
    let span = i128::from(to.since(from).micros());
    (0..count)
        .map(|i| {
            let step = span * i128::from(i) / i128::from(count);
            from + TimeDelta::from_micros(i64::try_from(step).unwrap_or(i64::MAX))
        })
        .collect()
}

/// A sheet about as wide as tall in frames: ⌈√n⌉ columns.
fn columns(count: u32) -> NonZeroU32 {
    let columns = (1..=count).find(|c| c * c >= count).unwrap_or(1);
    NonZeroU32::new(columns).unwrap_or(NonZeroU32::MIN)
}

fn png_path(cwd: &Path, out: &Path) -> Result<PathBuf, CliError> {
    match out.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("png") => {
            Ok(crate::export_args::resolved(&cwd.join(out)))
        }
        _ => Err(CliError::Usage(format!(
            "-o {}: картинка пишется в PNG, например -o кадр.png",
            out.display()
        ))),
    }
}

/// Renders with `render` and writes the PNG to `out`; a dry run only says what it would write.
fn write(
    ctx: &Ctx<'_>,
    workspace: &Workspace,
    out: &Path,
    text: String,
    mut json: serde_json::Value,
    render: impl FnOnce(
        &Gpu,
        &mut dc_render::Compositor,
        &mut Sources<'_>,
    ) -> Result<Vec<u8>, dc_encode::EncodeError>,
) -> Result<Answer, CliError> {
    json["dry_run"] = ctx.globals.dry_run.into();
    let mut answer = Answer::new(String::new(), json);
    answer.warnings.clone_from(&workspace.warnings);
    if ctx.globals.dry_run {
        answer.text = format!("dry-run, файл не записан: {text} → {}", out.display());
        return Ok(answer);
    }
    let failed = |e: &dyn std::fmt::Display| CliError::Failed(e.to_string());
    let gpu = Gpu::new(AdapterChoice::Auto).map_err(|e| failed(&e))?;
    let mut compositor = crate::scene::compositor(&gpu, workspace.dir().root())?;
    let mut sources = Sources::new(workspace.dir());
    let png = render(&gpu, &mut compositor, &mut sources).map_err(|e| failed(&e))?;
    std::fs::write(out, png).map_err(|e| {
        CliError::Failed(format!(
            "не удалось записать «{}» ({e}): проверь, что папка есть и доступна",
            out.display()
        ))
    })?;
    answer.text = format!("{text} → {}", out.display());
    Ok(answer)
}
