//! Options of `cargo xtask package`: which version and update channel the installer gets. CI packs
//! betas from `develop` as `<crate version>-beta.<run>` on the channel `beta` and releases from tags
//! on the default channel (CONTRIBUTING.md, «Выпуск»).

/// Velopack's default channel on Windows: its files are `releases.win.json`, `DevCast-win-Setup.exe`.
const DEFAULT_CHANNEL: &str = "win";

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub version: String,
    pub channel: String,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Options {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            channel: DEFAULT_CHANNEL.to_owned(),
        };
        let mut rest = args.iter();
        while let Some(key) = rest.next() {
            let value = rest
                .next()
                .ok_or_else(|| format!("после `{key}` нужно значение"))?;
            match key.as_str() {
                "--version" => options.version = checked_version(value)?,
                "--channel" => options.channel = checked_channel(value)?,
                other => {
                    return Err(format!(
                        "неизвестный флаг `{other}` (есть --version, --channel)"
                    ));
                }
            }
        }
        Ok(options)
    }

    /// The release index `vpk pack` writes for this channel.
    pub fn index(&self) -> String {
        format!("releases.{}.json", self.channel)
    }

    /// The Velopack installer `vpk pack` writes for this channel.
    pub fn velopack_setup(&self, pack_id: &str) -> String {
        format!("{pack_id}-{}-Setup.exe", self.channel)
    }
}

/// A SemVer 2 version without build metadata: `1.2.3` or `1.2.3-beta.4`.
fn checked_version(value: &str) -> Result<String, String> {
    let (core, pre) = value.split_once('-').unwrap_or((value, ""));
    let numbers: Vec<&str> = core.split('.').collect();
    let core_ok = numbers.len() == 3 && numbers.iter().all(|n| is_number(n));
    let pre_ok = value.contains('-').then(|| {
        pre.split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()))
    });
    match (core_ok, pre_ok) {
        (true, None | Some(true)) => Ok(value.to_owned()),
        _ => Err(format!(
            "версия `{value}` не по SemVer.\n  → Пиши как 1.2.3 или 1.2.3-beta.4."
        )),
    }
}

fn is_number(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| c.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'))
}

fn checked_channel(value: &str) -> Result<String, String> {
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        Ok(value.to_owned())
    } else {
        Err(format!(
            "канал `{value}` не подходит.\n  → Только строчные латинские буквы и цифры, например beta."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(&args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn without_flags_packs_the_crate_version_on_the_default_channel() {
        let options = parse(&[]).unwrap();
        assert_eq!(options.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(options.index(), "releases.win.json");
        assert_eq!(options.velopack_setup("DevCast"), "DevCast-win-Setup.exe");
    }

    #[test]
    fn a_beta_gets_its_own_channel_files() {
        let options = parse(&["--version", "0.2.0-beta.17", "--channel", "beta"]).unwrap();
        assert_eq!(options.version, "0.2.0-beta.17");
        assert_eq!(options.index(), "releases.beta.json");
        assert_eq!(options.velopack_setup("DevCast"), "DevCast-beta-Setup.exe");
    }

    #[test]
    fn rejects_what_velopack_would_not_order() {
        for bad in [
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.3-",
            "1.2.3-beta..1",
            "v1.2.3",
            "1.2.3+git",
        ] {
            assert!(parse(&["--version", bad]).is_err(), "{bad}");
        }
        assert!(parse(&["--channel", "Beta"]).is_err());
        assert!(parse(&["--version"]).is_err());
        assert!(parse(&["--stable", "x"]).is_err());
    }
}
