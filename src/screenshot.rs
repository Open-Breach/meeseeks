//! The screenshot button and the file it saves: what the virtual monitor
//! shows, as a PNG in the user's Pictures folder. It is handed a frame as a
//! texture and knows nothing about the pipeline or the portal.

use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::pictograms;

/// The folder in the Pictures folder that the screenshots go to.
const FOLDER: &str = "Meeseeks";

/// How long the button shows how a save went.
const FEEDBACK: Duration = Duration::from_millis(1500);

/// The button's faces, by their names in its stack.
const CAMERA: &str = "camera";
const SAVED: &str = "saved";
const NOT_SAVED: &str = "not-saved";

/// A frame on its way into a file. It is made when the button is clicked, so
/// the file is named after that moment, and saved when there is time for it.
pub struct Shot {
    texture: gdk::Texture,
    folder: PathBuf,
    stem: String,
}

impl Shot {
    pub fn new(texture: gdk::Texture) -> Self {
        Self {
            texture,
            folder: pictures().join(FOLDER),
            stem: stem(glib::DateTime::now_local().ok().as_ref()),
        }
    }

    /// Writes the frame as a PNG file and says where. The folder is made if
    /// it is not there, and a file that is there is left alone: the name
    /// gets a number instead.
    ///
    /// Reading the frame out and packing it takes a moment that grows with
    /// its size, so this is not for the main thread.
    pub fn save(self) -> io::Result<PathBuf> {
        // The folder first: without it there is no use in packing the frame.
        fs::create_dir_all(&self.folder).map_err(|err| at(&self.folder, &err))?;
        let png = self.texture.save_to_png_bytes();
        let (mut file, path) = create(&self.folder, &self.stem)?;
        if let Err(err) = file.write_all(&png) {
            // Half a picture is worse than none.
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(at(&path, &err));
        }
        Ok(path)
    }
}

/// An error that says which file or folder it is about.
fn at(path: &Path, err: &io::Error) -> io::Error {
    io::Error::new(err.kind(), format!("{}: {err}", path.display()))
}

/// The user's Pictures folder: the XDG user directory, which has another
/// name in many languages, or `~/Pictures` where none is set.
fn pictures() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Pictures)
        .unwrap_or_else(|| glib::home_dir().join("Pictures"))
}

/// The file's name without its extension: the date and the time, in an order
/// that sorts by time.
fn stem(now: Option<&glib::DateTime>) -> String {
    let time = now.and_then(|now| now.format("%Y-%m-%d %H-%M-%S").ok());
    match time {
        Some(time) => format!("Screenshot {time}"),
        None => "Screenshot".to_owned(),
    }
}

/// Makes a new file in `folder`, named `stem` and, if that name is taken,
/// ` (2)`, ` (3)` and so on after it. Never opens a file that is there.
fn create(folder: &Path, stem: &str) -> io::Result<(File, PathBuf)> {
    for number in 1u32.. {
        let name = match number {
            1 => format!("{stem}.png"),
            _ => format!("{stem} ({number}).png"),
        };
        let path = folder.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(at(&path, &err)),
        }
    }
    unreachable!("there are more numbers than files")
}

/// The button in the title bar: a camera, which turns into a tick for a
/// moment after a save and into the warning sign after one that failed.
pub struct Button {
    button: gtk::Button,
    faces: gtk::Stack,
    /// The timer that brings the camera back, while a mark is shown.
    turn_back: RefCell<Option<glib::SourceId>>,
}

impl Button {
    pub fn new() -> Rc<Self> {
        let faces = gtk::Stack::new();
        for (name, face) in [
            (CAMERA, pictograms::camera()),
            (SAVED, pictograms::saved()),
            (NOT_SAVED, pictograms::not_saved()),
        ] {
            face.set_valign(gtk::Align::Center);
            faces.add_named(&face, Some(name));
        }
        // A plain button of the desktop theme's, like Options.
        let button = gtk::Button::builder()
            .child(&faces)
            .tooltip_text("Save a screenshot")
            .visible(false)
            .build();
        button.update_property(&[gtk::accessible::Property::Label("Save a screenshot")]);
        Rc::new(Self {
            button,
            faces,
            turn_back: RefCell::default(),
        })
    }

    pub fn widget(&self) -> &gtk::Button {
        &self.button
    }

    /// Shows for a moment how a save went, then the camera again.
    pub fn show(self: &Rc<Self>, saved: bool) {
        self.faces
            .set_visible_child_name(if saved { SAVED } else { NOT_SAVED });
        let weak = Rc::downgrade(self);
        let timer = glib::timeout_add_local_once(FEEDBACK, move || {
            if let Some(button) = weak.upgrade() {
                // The timer is gone once it has run.
                button.turn_back.take();
                button.faces.set_visible_child_name(CAMERA);
            }
        });
        if let Some(earlier) = self.turn_back.replace(Some(timer)) {
            earlier.remove();
        }
    }

    /// The camera again, at once.
    pub fn reset(&self) {
        if let Some(timer) = self.turn_back.take() {
            timer.remove();
        }
        self.faces.set_visible_child_name(CAMERA);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of the test's own, which is removed again when it goes.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let folder =
                std::env::temp_dir().join(format!("meeseeks-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&folder);
            fs::create_dir_all(&folder).unwrap();
            Self(folder)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn named_after_the_date_and_time() {
        let time = glib::DateTime::from_local(2026, 11, 14, 16, 32, 5.0).unwrap();
        assert_eq!(stem(Some(&time)), "Screenshot 2026-11-14 16-32-05");
        assert_eq!(stem(None), "Screenshot");
    }

    #[test]
    fn a_name_that_is_taken_gets_a_number() {
        let scratch = Scratch::new("numbers");
        let names: Vec<String> = (0..3)
            .map(|_| {
                let (_, path) = create(&scratch.0, "Screenshot 2026-11-14 16-32-05").unwrap();
                path.file_name().unwrap().to_string_lossy().into_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "Screenshot 2026-11-14 16-32-05.png",
                "Screenshot 2026-11-14 16-32-05 (2).png",
                "Screenshot 2026-11-14 16-32-05 (3).png",
            ]
        );
    }

    #[test]
    fn a_file_that_is_there_is_left_alone() {
        let scratch = Scratch::new("kept");
        let taken = scratch.0.join("Screenshot.png");
        fs::write(&taken, b"mine").unwrap();
        let (mut file, path) = create(&scratch.0, "Screenshot").unwrap();
        file.write_all(b"new").unwrap();
        assert_eq!(path, scratch.0.join("Screenshot (2).png"));
        assert_eq!(fs::read(&taken).unwrap(), b"mine");
    }

    #[test]
    fn no_folder_no_file() {
        let scratch = Scratch::new("missing");
        let err = create(&scratch.0.join("not-there"), "Screenshot").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(err.to_string().contains("not-there"));
    }
}
