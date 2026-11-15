//! The settings file, `~/.config/meeseeks/settings.toml`. Knows nothing about
//! GTK, so that it can be read before GTK is initialised.

use std::path::PathBuf;
use std::{fs, io};

use serde::{Deserialize, Serialize};

use crate::renderer::Renderer;

/// The shortest and the longest side a virtual monitor can be asked to have.
pub const MIN_SIDE: i32 = 480;
pub const MAX_SIDE: i32 = 8192;

/// What the file holds. A key that is missing from it has its default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The renderer GTK is asked to draw with.
    pub renderer: Renderer,
    /// The `MEESEEKS_TRY` switch `copy`; `src/display.rs` says what it does.
    pub try_copy: bool,
    /// Whether the window keeps the size of the virtual monitor, so that the
    /// picture is always shown pixel for pixel.
    pub lock_window_size: bool,
    /// The size the virtual monitor was last started with.
    pub width: i32,
    pub height: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            renderer: Renderer::default(),
            try_copy: false,
            lock_window_size: true,
            width: 1920,
            height: 1080,
        }
    }
}

impl Settings {
    /// Reads the file. Without one the defaults apply, as they do if it
    /// cannot be understood, which is logged.
    pub fn load() -> Self {
        let Some(path) = path() else {
            return Self::default();
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(err) => {
                log::warn!("could not read {}: {err}", path.display());
                return Self::default();
            }
        };
        match toml::from_str::<Self>(&text) {
            Ok(settings) => settings.sane(),
            Err(err) => {
                log::warn!("{} is ignored: {err}", path.display());
                Self::default()
            }
        }
    }

    /// Writes the file, by way of a temporary one so that a write cut short
    /// leaves the old file behind.
    pub fn save(&self) -> io::Result<()> {
        let path = path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "neither XDG_CONFIG_HOME nor HOME is set",
            )
        })?;
        let text = toml::to_string(self).map_err(io::Error::other)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temporary = path.with_extension("toml.new");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, &path)
    }

    /// The switches that are on, the way `MEESEEKS_TRY` lists them.
    pub fn tries(&self) -> String {
        [(self.try_copy, "copy")]
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// A size out of range was not written by this app; it goes back to the
    /// default.
    fn sane(mut self) -> Self {
        let sides = MIN_SIDE..=MAX_SIDE;
        if !sides.contains(&self.width) || !sides.contains(&self.height) {
            let default = Self::default();
            self.width = default.width;
            self.height = default.height;
        }
        self
    }
}

/// `$XDG_CONFIG_HOME/meeseeks/settings.toml`, with `~/.config` standing in
/// for an unset or relative `XDG_CONFIG_HOME`.
fn path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })?;
    Some(config.join("meeseeks").join("settings.toml"))
}
