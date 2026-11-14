//! Where the settings meet the environment variables that carry them. GTK
//! takes its renderer from `GSK_RENDERER` and `GDK_DISABLE` when it is
//! initialised, and the pipeline its switches from `MEESEEKS_TRY`. A variable
//! that is set already wins over the settings file.

use std::ffi::OsString;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use crate::renderer::{self, Renderer};
use crate::settings::Settings;

/// The variables the settings can go into.
const VARIABLES: [&str; 3] = ["GSK_RENDERER", "GDK_DISABLE", "MEESEEKS_TRY"];

/// What became of the settings on their way into the environment.
pub struct Environment {
    /// The variables as they were found, for [`Environment::relaunch`].
    found: [(&'static str, Option<OsString>); 3],
    /// The renderer GTK has been asked for, if it is one this app knows.
    pub asked: Option<Renderer>,
    /// Whether `GSK_RENDERER` was set, so that the renderer in the settings
    /// is not the one in use.
    pub renderer_overridden: bool,
    /// The same for `MEESEEKS_TRY` and the switches.
    pub tries_overridden: bool,
}

/// Puts the renderer and the switches from `settings` into the environment,
/// each unless its variable is set already.
///
/// # Safety
///
/// This changes the environment, so it has to run before any other thread
/// exists that could be reading it: at the start of `main`, before GTK.
pub unsafe fn apply(settings: &Settings) -> Environment {
    let found = VARIABLES.map(|name| (name, std::env::var_os(name)));
    let renderer_overridden = std::env::var_os("GSK_RENDERER").is_some();
    let tries_overridden = std::env::var_os("MEESEEKS_TRY").is_some();

    if renderer_overridden {
        log::info!("GSK_RENDERER is set; it is used instead of the renderer in the settings");
    }
    // SAFETY: the caller vouches for there being no other thread.
    let asked = unsafe { renderer::request(settings.renderer) };

    let tries = settings.tries();
    if tries_overridden {
        log::info!("MEESEEKS_TRY is set; it is used instead of the switches in the settings");
    } else if !tries.is_empty() {
        // SAFETY: as above.
        unsafe { std::env::set_var("MEESEEKS_TRY", tries) };
    }

    Environment {
        found,
        asked,
        renderer_overridden,
        tries_overridden,
    }
}

impl Environment {
    /// Replaces this process with a fresh start of the app, which reads the
    /// settings anew. It gets the environment as it was found: with the
    /// settings of this run still in it, they would pass for variables set
    /// from outside and win over the new ones. Returns only if it fails.
    pub fn relaunch(&self) -> io::Error {
        let mut arguments = std::env::args_os();
        let invoked_as = arguments.next().map(PathBuf::from);
        let Some(program) = std::env::current_exe().ok().or(invoked_as) else {
            return io::Error::other("the path of the program is not known");
        };
        let mut command = Command::new(program);
        command.args(arguments);
        for (name, value) in &self.found {
            match value {
                Some(value) => command.env(name, value),
                None => command.env_remove(name),
            };
        }
        command.exec()
    }
}
