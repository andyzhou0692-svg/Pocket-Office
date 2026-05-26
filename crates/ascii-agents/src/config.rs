use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub theme: Option<String>,
    #[serde(rename = "max-desks")]
    pub max_desks: Option<usize>,
}

pub fn config_path() -> PathBuf {
    if let Ok(base) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(base).join("ascii-agents").join("config.toml");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("ascii-agents")
            .join("config.toml");
    }
    PathBuf::from(".config/ascii-agents/config.toml")
}

pub fn load(path: &Path) -> AppConfig {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return AppConfig::default(),
        Err(e) => {
            tracing::warn!(path = %path.display(), %e, "cannot read config — using defaults");
            return AppConfig::default();
        }
    };
    match toml::from_str(&contents) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(path = %path.display(), %e, "malformed config — using defaults");
            AppConfig::default()
        }
    }
}

pub fn save(path: &Path, theme_name: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = path.with_extension("toml.lock");
    let lock_file = std::fs::File::create(&lock_path)?;
    fs2::FileExt::lock_exclusive(&lock_file)?;

    let mut cfg = if path.exists() {
        load(path)
    } else {
        AppConfig::default()
    };
    cfg.theme = Some(theme_name.to_string());

    let contents = toml::to_string_pretty(&cfg)?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, &contents)?;
    std::fs::rename(&tmp, path)?;
    // Lock released on drop
    Ok(())
}

pub fn resolve(
    config: AppConfig,
    cli_theme: Option<String>,
    cli_max_desks: Option<usize>,
) -> (String, usize) {
    let config_theme = config.theme.filter(|t| {
        if crate::tui::theme::theme_by_name(t).is_some() {
            true
        } else {
            tracing::warn!(theme = %t, "unknown theme in config — ignoring");
            false
        }
    });
    let theme = cli_theme
        .or(config_theme)
        .unwrap_or_else(|| "normal".to_string());
    let max_desks = cli_max_desks.or(config.max_desks).unwrap_or(16);
    (theme, max_desks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_defaults() {
        let cfg = load(Path::new("/nonexistent/path/config.toml"));
        assert!(cfg.theme.is_none());
        assert!(cfg.max_desks.is_none());
    }

    #[test]
    fn load_malformed_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "not valid { toml }}}").unwrap();
        let cfg = load(&path);
        assert!(cfg.theme.is_none());
    }

    #[test]
    fn load_partial_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = \"cyberpunk\"\n").unwrap();
        let cfg = load(&path);
        assert_eq!(cfg.theme.as_deref(), Some("cyberpunk"));
        assert!(cfg.max_desks.is_none());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        save(&path, "dracula").unwrap();
        let cfg = load(&path);
        assert_eq!(cfg.theme.as_deref(), Some("dracula"));
    }

    #[test]
    fn save_preserves_max_desks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = \"normal\"\nmax-desks = 8\n").unwrap();
        save(&path, "cyberpunk").unwrap();
        let cfg = load(&path);
        assert_eq!(cfg.theme.as_deref(), Some("cyberpunk"));
        assert_eq!(cfg.max_desks, Some(8));
    }

    #[test]
    fn resolve_cli_wins_over_config() {
        let cfg = AppConfig {
            theme: Some("normal".into()),
            max_desks: Some(8),
        };
        let (theme, desks) = resolve(cfg, Some("dracula".into()), Some(4));
        assert_eq!(theme, "dracula");
        assert_eq!(desks, 4);
    }

    #[test]
    fn resolve_config_wins_over_default() {
        let cfg = AppConfig {
            theme: Some("gruvbox".into()),
            max_desks: Some(12),
        };
        let (theme, desks) = resolve(cfg, None, None);
        assert_eq!(theme, "gruvbox");
        assert_eq!(desks, 12);
    }

    #[test]
    fn resolve_all_none_uses_default() {
        let cfg = AppConfig::default();
        let (theme, desks) = resolve(cfg, None, None);
        assert_eq!(theme, "normal");
        assert_eq!(desks, 16);
    }

    #[test]
    fn resolve_invalid_config_theme_falls_back_to_default() {
        let cfg = AppConfig {
            theme: Some("does-not-exist".into()),
            max_desks: Some(8),
        };
        let (theme, desks) = resolve(cfg, None, None);
        assert_eq!(theme, "normal");
        assert_eq!(desks, 8);
    }

    #[test]
    fn resolve_mixed_cli_theme_config_max_desks() {
        let cfg = AppConfig {
            theme: Some("gruvbox".into()),
            max_desks: Some(8),
        };
        let (theme, desks) = resolve(cfg, Some("dracula".into()), None);
        assert_eq!(theme, "dracula");
        assert_eq!(desks, 8);
    }

    #[test]
    fn full_config_flow_file_drives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = \"cyberpunk\"\nmax-desks = 8\n").unwrap();
        let cfg = load(&path);
        let (theme, desks) = resolve(cfg, None, None);
        assert_eq!(theme, "cyberpunk");
        assert_eq!(desks, 8);
    }

    #[test]
    fn full_config_flow_cli_partially_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "theme = \"cyberpunk\"\nmax-desks = 8\n").unwrap();
        let cfg = load(&path);
        let (theme, desks) = resolve(cfg, Some("dracula".into()), None);
        assert_eq!(theme, "dracula");
        assert_eq!(desks, 8);
    }
}
