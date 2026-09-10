//! Responsive, controller-accessible asset import screen. All disc IO happens
//! on a cancellable worker. SDL dialogs only communicate via a channel.

use super::menu::{MenuRow, MenuStatus, MenuView};
use extraction::{self, Progress, Source};
use menus::{MenuState, Unlocks, controller::Controllers, input};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportAction {
    Search,
    Browse,
    Back,
    Cancel,
}

enum Message {
    Progress(Progress),
    Finished(Result<PathBuf, ImportError>),
    Picked(Result<Option<PathBuf>, String>),
}

struct ImportError {
    message: String,
    manual_search: bool,
}

pub struct AssetImportMenu {
    destination: Result<PathBuf, String>,
    roots: Vec<PathBuf>,
    selected: usize,
    controllers: Controllers,
    sender: Sender<Message>,
    receiver: Receiver<Message>,
    worker: Option<JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
    picking: bool,
    message: String,
    percent: Option<u8>,
}

impl AssetImportMenu {
    pub fn new(destination: Result<PathBuf, String>, roots: Vec<PathBuf>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let message = match &destination {
            Err(error) => error.clone(),
            Ok(path) if path.exists() => "Assets already exist. Find ISO verifies the installed files without needing the disc.".into(),
            Ok(_) => "Import an unmodified Melee USA 1.02 ISO. Needs about 1.5 GB free. Visual conversion is not yet available in-game.".into(),
        };
        Self {
            destination,
            roots,
            selected: 0,
            controllers: Controllers::default(),
            sender,
            receiver,
            worker: None,
            cancel: Arc::new(AtomicBool::new(false)),
            picking: false,
            message,
            percent: None,
        }
    }

    pub fn busy(&self) -> bool {
        self.worker.is_some() || self.picking
    }

    pub fn release_input(&mut self) {
        self.controllers.poll([0; 4]);
    }

    pub fn tick(&mut self, held: [u32; 4]) -> Option<ImportAction> {
        let frames = self.controllers.poll(held);
        let buttons = input::decode(input::aggregate(&frames));
        if self.busy() {
            return (buttons & (input::BACK | input::CONFIRM) != 0 && !self.picking)
                .then_some(ImportAction::Cancel);
        }
        if buttons & input::BACK != 0 {
            return Some(ImportAction::Back);
        }
        if buttons & input::CONFIRM != 0 {
            return Some(
                [
                    ImportAction::Search,
                    ImportAction::Browse,
                    ImportAction::Back,
                ][self.selected],
            );
        }
        if buttons & input::UP != 0 {
            self.selected = (self.selected + 2) % 3;
        } else if buttons & input::DOWN != 0 {
            self.selected = (self.selected + 1) % 3;
        }
        None
    }

    pub fn start_search(&mut self) {
        self.start(Source::Search(self.roots.clone()));
    }

    pub fn start(&mut self, source: Source) {
        if self.busy() {
            return;
        }
        let destination = match &self.destination {
            Ok(path) => path.clone(),
            Err(error) => {
                self.message = error.clone();
                return;
            }
        };
        self.cancel.store(false, Ordering::Relaxed);
        let cancel = Arc::clone(&self.cancel);
        let sender = self.sender.clone();
        self.message = "Looking for a valid ISO...".into();
        self.percent = None;
        self.selected = 0;
        match thread::Builder::new()
            .name("asset-import".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    extraction::import(source, &destination, &cancel, |progress| {
                        let _ = sender.send(Message::Progress(progress));
                    })
                    .map(|bundle| bundle.root().to_owned())
                    .map_err(|error| ImportError {
                        manual_search: error.is::<extraction::IsoNotFound>(),
                        message: format!("{error:#}"),
                    })
                }))
                .unwrap_or_else(|_| {
                    Err(ImportError {
                        message: "Import stopped unexpectedly. Retry the import.".into(),
                        manual_search: false,
                    })
                });
                let _ = sender.send(Message::Finished(result));
            }) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => self.message = format!("Could not start import: {error}"),
        }
    }

    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.message = "Cancelling import and removing incomplete files...".into();
    }

    /// Pass this callback to SDL on the window thread. Captures no SDL objects.
    pub fn dialog_callback(&mut self) -> sdl3::dialog::DialogCallback {
        self.picking = true;
        self.message = "Choose your ISO in the file picker. Cancel there to return.".into();
        let sender = self.sender.clone();
        Box::new(move |result, _| {
            let result = match result {
                Ok(files) => Ok(files.into_iter().next()),
                Err(sdl3::dialog::DialogError::Canceled) => Ok(None),
                Err(error) => Err(format!(
                    "File picker unavailable: {error}. Try Find ISO automatically, or drop an ISO onto the game window."
                )),
            };
            let _ = sender.send(Message::Picked(result));
        })
    }

    pub fn dialog_failed(&mut self, error: impl std::fmt::Display) {
        self.picking = false;
        self.message = format!("Could not open the file picker: {error}");
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(message) = self.receiver.try_recv() {
            changed = true;
            match message {
                Message::Picked(result) => {
                    self.picking = false;
                    match result {
                        Ok(Some(path)) => self.start(Source::File(path)),
                        Ok(None) => self.message = "File selection cancelled.".into(),
                        Err(error) => self.message = error,
                    }
                }
                Message::Progress(progress) => {
                    if self.cancel.load(Ordering::Relaxed) {
                        continue;
                    }
                    match progress {
                        Progress::Searching => {
                            self.message =
                                "Searching Downloads, Games, and local game storage...".into();
                            self.percent = None;
                        }
                        Progress::Validating { percent } => {
                            self.message = format!("Checking ISO integrity: {percent}%");
                            self.percent = Some(percent);
                        }
                        Progress::Extracting { completed, total } => {
                            self.message = format!("Importing game files: {completed} / {total}");
                            self.percent = Some((completed * 100 / total.max(1)) as u8);
                        }
                        Progress::CheckingInstalled { completed, total } => {
                            self.message =
                                format!("Verifying installed files: {completed} / {total}");
                            self.percent = Some((completed * 100 / total.max(1)) as u8);
                        }
                    }
                }
                Message::Finished(result) => {
                    if let Some(worker) = self.worker.take() {
                        let _ = worker.join();
                    }
                    self.percent = None;
                    self.message = match result {
                        Ok(path) => format!(
                            "Game files installed in {}. Visual conversion and gameplay integration are not yet available in-game.",
                            path.display()
                        ),
                        Err(error) => {
                            if error.manual_search {
                                self.selected = 1;
                            }
                            eprintln!("Asset import: {}", error.message);
                            error.message
                        }
                    };
                }
            }
        }
        changed
    }

    pub fn view(&self) -> MenuView {
        let mut view = MenuView::from(MenuState::new(Unlocks::default()).snapshot());
        view.title = "Import Game Assets";
        view.breadcrumb = "HOME / IMPORT GAME ASSETS";
        let labels: &[&str] = if self.picking {
            &["Waiting for file picker..."]
        } else if self.worker.is_some() {
            &["Cancel import"]
        } else {
            &["Find ISO automatically", "Choose ISO file...", "Back"]
        };
        view.rows = labels
            .iter()
            .enumerate()
            .map(|(index, &label)| MenuRow {
                index: index as u16,
                label,
                selected: index == self.selected,
            })
            .collect();
        view.selected_label = labels[self.selected.min(labels.len() - 1)];
        view.status = Some(MenuStatus {
            lines: wrap(&self.message),
            percent: self.percent,
        });
        view
    }
}

impl Drop for AssetImportMenu {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn wrap(text: &str) -> Vec<String> {
    // Character-based chunks also handle long paths without overflowing the UI.
    text.chars()
        .collect::<Vec<_>>()
        .chunks(66)
        .take(6)
        .map(|chunk| chunk.iter().collect())
        .collect()
}
