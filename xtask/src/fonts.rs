//! Fonts of the renderer in `.deps/fonts/` (docs/decisions/005-text.md): official files of the
//! google/fonts repository at a pinned commit, and Noto Color Emoji in its colour-bitmap (CBDT)
//! build from the googlefonts/noto-emoji release (google/fonts has only the COLRv1 build, which
//! swash does not draw). Each file is checked by SHA-256; their git blob hashes were checked
//! against the repositories when the list was made.

use std::path::Path;
use std::process::Command;

use crate::native::{create_dir, run_tool, sha256_file};

const OFL: &str =
    "https://raw.githubusercontent.com/google/fonts/9862abc066c9b83bf84c3e8ed1fc2d8d48f9bbf0/ofl";
const EMOJI: &str = "https://raw.githubusercontent.com/googlefonts/noto-emoji/v2.051/fonts";

struct Font {
    repo: &'static str,
    /// Path in the repository.
    path: &'static str,
    /// Name in `.deps/fonts/`.
    file: &'static str,
    sha256: &'static str,
}

const fn font(path: &'static str, file: &'static str, sha256: &'static str) -> Font {
    Font {
        repo: OFL,
        path,
        file,
        sha256,
    }
}

const FONTS: [Font; 19] = [
    font(
        "inter/Inter%5Bopsz,wght%5D.ttf",
        "Inter[opsz,wght].ttf",
        "29160a80ff49ddcab2c97711247e08b1fab27a484a329ce8b813d820dc559031",
    ),
    font(
        "inter/Inter-Italic%5Bopsz,wght%5D.ttf",
        "Inter-Italic[opsz,wght].ttf",
        "acd98e64795781b2058f07b18475e0ecee2a0fe2b42a49e2f9e37d0d6bf66ce6",
    ),
    font(
        "inter/OFL.txt",
        "Inter-OFL.txt",
        "5b9321a4298cfeb6b34354164a1c3afc3db114569984c502b9b35d988fd58c57",
    ),
    font(
        "montserrat/Montserrat%5Bwght%5D.ttf",
        "Montserrat[wght].ttf",
        "0f7b311b2f3279e4eef9b2f968bcdbab6e28f4daeb1f049f4f278a902bcd82f7",
    ),
    font(
        "montserrat/Montserrat-Italic%5Bwght%5D.ttf",
        "Montserrat-Italic[wght].ttf",
        "51607f316bc020e59f03cbf51543eecffbea501c0b31d73e5b82927c5cca442c",
    ),
    font(
        "montserrat/OFL.txt",
        "Montserrat-OFL.txt",
        "8b7141c03fa4f8d44e6345d5d4931709290f0f67875e452e95ac1fd3a027802e",
    ),
    font(
        "roboto/Roboto%5Bwdth,wght%5D.ttf",
        "Roboto[wdth,wght].ttf",
        "d7598e12c5dbef095ff8272cfc55da0250bd07fbdecbac8a530b9b277872a134",
    ),
    font(
        "roboto/Roboto-Italic%5Bwdth,wght%5D.ttf",
        "Roboto-Italic[wdth,wght].ttf",
        "9725a847af6b460ffca162ae66d20dad48b01876137947180b42d7dcd7887182",
    ),
    font(
        "roboto/OFL.txt",
        "Roboto-OFL.txt",
        "061402327a96aadb0bfb694a960ed289ecd38d383e396243831ab81feb109c41",
    ),
    font(
        "ptserif/PT_Serif-Web-Regular.ttf",
        "PT_Serif-Web-Regular.ttf",
        "a4951fade06ff8f09b7673aa81ffb65a8cd409e24d3289a6dc670bc4dda2557a",
    ),
    font(
        "ptserif/PT_Serif-Web-Bold.ttf",
        "PT_Serif-Web-Bold.ttf",
        "038ba7336bd7ea14f12ad155bed51a4345cac5153275d521dec3ba04021c526e",
    ),
    font(
        "ptserif/PT_Serif-Web-Italic.ttf",
        "PT_Serif-Web-Italic.ttf",
        "f57e95ff9dc85691a3b2e193f2028db36f6663939a46c0fc4f286d618b80b7ce",
    ),
    font(
        "ptserif/PT_Serif-Web-BoldItalic.ttf",
        "PT_Serif-Web-BoldItalic.ttf",
        "f003788ba08981eb0988b3557a6f224a53dab49c20e283e8b74d5af3c466f8be",
    ),
    font(
        "ptserif/OFL.txt",
        "PT_Serif-OFL.txt",
        "ddf311c28ddf5a5ad9747649837346b67bed9d356789c3072bb27dbce49e514d",
    ),
    font(
        "jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf",
        "JetBrainsMono[wght].ttf",
        "48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda",
    ),
    font(
        "jetbrainsmono/JetBrainsMono-Italic%5Bwght%5D.ttf",
        "JetBrainsMono-Italic[wght].ttf",
        "85ae2a5cd3f56baf1ce1c21a851322c58e3d8fbe8e8ad4a4d090a820dd7fe558",
    ),
    font(
        "jetbrainsmono/OFL.txt",
        "JetBrainsMono-OFL.txt",
        "b2fe5e8987594e9ffd1d2ca52a2f5d73eb8335243893c5d6254b5ad69269591d",
    ),
    Font {
        repo: EMOJI,
        path: "NotoColorEmoji.ttf",
        file: "NotoColorEmoji.ttf",
        sha256: "72a635cb3d2f3524c51620cdde406b217204e8a6a06c6a096ff8ed4b5fd6e27b",
    },
    Font {
        repo: EMOJI,
        path: "LICENSE",
        file: "NotoColorEmoji-OFL.txt",
        sha256: "6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2",
    },
];

pub fn ensure(deps: &Path) -> Result<(), String> {
    let dest = deps.join("fonts");
    create_dir(&dest)?;
    for font in &FONTS {
        let file = dest.join(font.file);
        if file.exists() && sha256_file(&file)? == font.sha256 {
            continue;
        }
        println!("fonts: скачиваю {}", font.file);
        let url = format!("{}/{}", font.repo, font.path);
        run_tool(
            Command::new("curl")
                .args(["-fsSL", "--retry", "3", "-o"])
                .arg(&file)
                .arg(&url),
        )?;
        let actual = sha256_file(&file)?;
        if actual != font.sha256 {
            std::fs::remove_file(&file)
                .map_err(|e| format!("не удалось удалить {}: {e}", file.display()))?;
            return Err(format!(
                "{}: SHA-256 не совпал (ожидался {}, получен {actual}).\n  → Файл повреждён или подменён; повтори, а если повторяется — не используй его.",
                font.file, font.sha256
            ));
        }
    }
    println!("fonts: OK — {} файлов в .deps/fonts", FONTS.len());
    Ok(())
}

/// The fonts with their licences next to the built programs (`<profile>/fonts`) and tests
/// (`<profile>/deps/fonts`): the programs load them from there (`Compositor::load_builtin_fonts`).
pub fn copy(deps: &Path, target: &Path) -> Result<(), String> {
    let source = deps.join("fonts");
    for profile in ["debug", "release"] {
        for dir in [
            target.join(profile).join("fonts"),
            target.join(profile).join("deps").join("fonts"),
        ] {
            create_dir(&dir)?;
            for font in &FONTS {
                std::fs::copy(source.join(font.file), dir.join(font.file))
                    .map_err(|e| format!("не удалось скопировать {}: {e}", font.file))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_font_has_a_full_hash_and_a_licence() {
        for font in &FONTS {
            assert_eq!(font.sha256.len(), 64, "{}", font.file);
        }
        let family = |file: &str| file.split(['[', '-', '.']).next().map(str::to_owned);
        for font in FONTS.iter().filter(|f| f.file.ends_with(".ttf")) {
            assert!(
                FONTS
                    .iter()
                    .any(|f| f.file.ends_with("OFL.txt") && family(f.file) == family(font.file)),
                "{}: нет файла лицензии",
                font.file
            );
        }
    }
}
