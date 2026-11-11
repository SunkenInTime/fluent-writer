//! Which copy of the app is running. The dev build keeps its own settings, drafts and support files,
//! so trying a new build never touches the copy you write with every day.

use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppVariant {
    #[default]
    Standard,
    Dev,
}

impl AppVariant {
    /// The raw value Swift used (`FWVariant` in Info.plist): `standard` or `dev`.
    pub fn raw_value(self) -> &'static str {
        match self {
            AppVariant::Standard => "standard",
            AppVariant::Dev => "dev",
        }
    }

    pub fn app_name(self) -> &'static str {
        match self {
            AppVariant::Dev => "Fluent Writer Dev",
            AppVariant::Standard => "Fluent Writer",
        }
    }

    fn folder_name(self) -> &'static str {
        self.app_name()
    }

    /// Settings, state, recovery journals and notes.
    ///
    /// macOS: `~/Library/Application Support/<app name>` (as the Swift app).
    /// Linux: `$XDG_DATA_HOME/<app name>` (usually `~/.local/share`).
    /// Windows: `%APPDATA%\<app name>`.
    pub fn support_directory(self) -> PathBuf {
        let base = dirs::data_dir().unwrap_or_else(|| home().join(".local").join("share"));
        base.join(self.folder_name())
    }

    /// Where new drafts are written.
    ///
    /// macOS and Windows: `~/Documents/<app name>`. Linux: `$XDG_DOCUMENTS_DIR/<app name>`,
    /// falling back to `~/Documents`.
    pub fn drafts_directory(self) -> PathBuf {
        let base = dirs::document_dir().unwrap_or_else(|| home().join("Documents"));
        base.join(self.folder_name())
    }
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

impl FromStr for AppVariant {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "standard" => Ok(AppVariant::Standard),
            "dev" => Ok(AppVariant::Dev),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_folders() {
        assert_eq!(AppVariant::Dev.app_name(), "Fluent Writer Dev");
        assert!(
            AppVariant::Standard
                .drafts_directory()
                .ends_with("Fluent Writer")
        );
        assert!(
            AppVariant::Dev
                .support_directory()
                .ends_with("Fluent Writer Dev")
        );
        assert_eq!("dev".parse::<AppVariant>(), Ok(AppVariant::Dev));
        assert!("other".parse::<AppVariant>().is_err());
        #[cfg(target_os = "macos")]
        {
            let home = dirs::home_dir().unwrap();
            assert_eq!(
                AppVariant::Standard.support_directory(),
                home.join("Library/Application Support/Fluent Writer")
            );
            assert_eq!(
                AppVariant::Dev.drafts_directory(),
                home.join("Documents/Fluent Writer Dev")
            );
        }
    }
}
