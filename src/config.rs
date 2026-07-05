use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub model: Option<String>,
}

pub fn config_path() -> Option<PathBuf> {
    config_path_from(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        dirs::home_dir(),
    )
}

pub fn config_path_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = xdg.or_else(|| home.map(|h| h.join(".config")))?;
    Some(base.join("is").join("config.toml"))
}

pub fn load(path: &Path) -> Config {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Config::default();
    };
    match toml::from_str(&text) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("warning: ignoring malformed config {}: {err}", path.display());
            Config::default()
        }
    }
}

pub fn save(path: &Path, config: &Config) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(config).expect("config always serializes");
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_prefers_xdg_config_home() {
        let p = config_path_from(Some(PathBuf::from("/xdg")), Some(PathBuf::from("/home/u")));
        assert_eq!(p, Some(PathBuf::from("/xdg/is/config.toml")));
    }

    #[test]
    fn path_falls_back_to_home_dot_config() {
        let p = config_path_from(None, Some(PathBuf::from("/home/u")));
        assert_eq!(p, Some(PathBuf::from("/home/u/.config/is/config.toml")));
        assert_eq!(config_path_from(None, None), None);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&dir.path().join("nope.toml")), Config::default());
    }

    #[test]
    fn save_then_load_roundtrips_and_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep/is/config.toml");
        let cfg = Config { model: Some("sonnet".into()) };
        save(&path, &cfg).unwrap();
        assert_eq!(load(&path), cfg);
    }

    #[test]
    fn load_ignores_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "model = \"opus\"\nfuture_knob = 3\n").unwrap();
        assert_eq!(load(&path), Config { model: Some("opus".into()) });
    }

    #[test]
    fn load_malformed_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "model = [not toml").unwrap();
        assert_eq!(load(&path), Config::default());
    }
}
