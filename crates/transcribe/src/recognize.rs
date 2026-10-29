//! Speech recognition with whisper-rs (whisper.cpp): on the first Vulkan GPU, on the CPU when
//! there is none. Word times by DTW with flash attention off (decision 010).

use std::path::Path;

use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters, WhisperState,
};

use crate::error::{Result, TranscribeError};
use crate::model::Model;
use crate::words::{Segment, Token, Word, words};

/// Sample rate Whisper takes.
pub const WHISPER_RATE: u32 = 16_000;

/// Whisper times are in centiseconds.
const CENTI: i64 = 10_000;

/// What was said and in which language.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Recognized {
    /// Language code, e.g. `ru`.
    pub language: String,
    pub words: Vec<Word>,
}

/// Recognizes `samples` (mono, [`WHISPER_RATE`]) with the model file `path` of kind `model`.
/// `language` — a code such as `ru`, or `None` to detect it. `progress` gets percents.
///
/// No cancellation: `set_abort_callback_safe` of whisper-rs 0.16 casts its data to the wrong
/// type, and whisper.cpp then stops every run with «failed to encode» (docs/QUESTIONS.md).
pub fn recognize(
    path: &Path,
    model: Model,
    samples: &[f32],
    language: Option<&str>,
    progress: impl FnMut(i32) + 'static,
) -> Result<Recognized> {
    let whisper = |e: whisper_rs::WhisperError| TranscribeError::Whisper(e.to_string());
    // whisper.cpp and ggml print to stderr; without a log backend this turns them off.
    whisper_rs::install_logging_hooks();
    let mut context = WhisperContextParameters::default();
    context.flash_attn(false);
    let preset = match model {
        Model::Small => DtwModelPreset::Small,
        Model::LargeV3Turbo => DtwModelPreset::LargeV3Turbo,
    };
    context.dtw_parameters(DtwParameters {
        mode: DtwMode::ModelPreset {
            model_preset: preset,
        },
        ..Default::default()
    });
    let file = path
        .to_str()
        .ok_or_else(|| TranscribeError::ModelPath(path.to_path_buf()))?;
    let context = WhisperContext::new_with_params(file, context).map_err(whisper)?;
    let mut state = context.create_state().map_err(whisper)?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // One thread per physical core: with a thread per logical core ggml waits on its spinning
    // barriers, and a busy 8-core i7 recognized 10 s of speech in 43 s instead of 4 s.
    params.set_n_threads(num_cpus::get_physical() as i32);
    params.set_language(Some(language.unwrap_or("auto")));
    params.set_token_timestamps(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_progress_callback_safe(progress);
    state.full(params, samples).map_err(whisper)?;

    let segments = segments(&state, context.token_eot())?;
    let id = state.full_lang_id_from_state();
    let language = whisper_rs::get_lang_str(id)
        .ok_or_else(|| TranscribeError::Whisper(format!("неизвестный язык с номером {id}")))?
        .to_owned();
    Ok(Recognized {
        language,
        words: words(&segments),
    })
}

/// Tokens of every recognized segment, without special ones (`eot` and above).
fn segments(state: &WhisperState, eot: i32) -> Result<Vec<Segment>> {
    let whisper = |e: whisper_rs::WhisperError| TranscribeError::Whisper(e.to_string());
    let mut segments = Vec::new();
    for segment in state.as_iter() {
        let mut tokens = Vec::new();
        for i in 0..segment.n_tokens() {
            let Some(token) = segment.get_token(i) else {
                continue;
            };
            if token.token_id() >= eot {
                continue;
            }
            let data = token.token_data();
            tokens.push(Token {
                text: token.to_str_lossy().map_err(whisper)?.into_owned(),
                dtw: if data.t_dtw < 0 {
                    -1
                } else {
                    data.t_dtw * CENTI
                },
                t0: data.t0 * CENTI,
                probability: token.token_probability(),
            });
        }
        segments.push(Segment {
            tokens,
            end: segment.end_timestamp() * CENTI,
        });
    }
    Ok(segments)
}
