use std::{
    env,
    error::Error,
    fs,
    num::NonZeroU32,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::egui;
use librqbit::{AddTorrent, Session};
use tokio::sync::mpsc::{self as async_mpsc, UnboundedReceiver, UnboundedSender};

enum Command {
    Pause,
    Resume,
    Remove,
    SetLimit(Option<NonZeroU32>),
}

enum Outcome {
    Completed,
    Removed,
}

enum Event {
    Progress { downloaded: u64, total: u64 },
    Paused(bool),
    LimitApplied,
    Finished(Result<Outcome, String>),
}

struct TorrentApp {
    source: String,
    output: String,
    status: String,
    downloaded: u64,
    total: u64,
    limit_kib: u32,
    running: bool,
    paused: bool,
    action_pending: bool,
    events: Option<Receiver<Event>>,
    commands: Option<UnboundedSender<Command>>,
}

impl Default for TorrentApp {
    fn default() -> Self {
        let output = env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Downloads");
        Self {
            source: String::new(),
            output: output.display().to_string(),
            status: "Оберіть торрент або вставте magnet-посилання".into(),
            downloaded: 0,
            total: 0,
            limit_kib: 0,
            running: false,
            paused: false,
            action_pending: false,
            events: None,
            commands: None,
        }
    }
}

impl TorrentApp {
    fn start(&mut self) {
        let source = self.source.trim().to_owned();
        let output = self.output.trim().to_owned();
        if source.is_empty() || output.is_empty() {
            self.status = "Вкажіть торрент і папку для завантаження".into();
            return;
        }

        let (sender, receiver) = mpsc::channel();
        let (commands, command_receiver) = async_mpsc::unbounded_channel();
        let limit = self.limit_bps();
        thread::spawn(move || download(source, output, limit, command_receiver, sender));
        self.events = Some(receiver);
        self.commands = Some(commands);
        self.running = true;
        self.paused = false;
        self.action_pending = false;
        self.downloaded = 0;
        self.total = 0;
        self.status = "Підключення до пірів…".into();
    }

    fn limit_bps(&self) -> Option<NonZeroU32> {
        self.limit_kib.checked_mul(1024).and_then(NonZeroU32::new)
    }

    fn send_command(&mut self, command: Command) {
        if let Some(commands) = &self.commands {
            if commands.send(command).is_ok() {
                self.action_pending = true;
            } else {
                self.status = "Не вдалося передати команду завантаженню".into();
            }
        }
    }

    fn read_events(&mut self) {
        let mut finished = false;
        if let Some(receiver) = &self.events {
            while let Ok(event) = receiver.try_recv() {
                match event {
                    Event::Progress { downloaded, total } => {
                        self.downloaded = downloaded;
                        self.total = total;
                        if !self.paused && !self.action_pending {
                            self.status = "Завантаження…".into();
                        }
                    }
                    Event::Paused(paused) => {
                        self.paused = paused;
                        self.action_pending = false;
                        self.status = if paused {
                            "На паузі".into()
                        } else {
                            "Завантаження…".into()
                        };
                    }
                    Event::LimitApplied => {
                        self.action_pending = false;
                        self.status = if self.paused {
                            "На паузі".into()
                        } else {
                            "Ліміт швидкості застосовано".into()
                        };
                    }
                    Event::Finished(result) => {
                        self.status = match result {
                            Ok(Outcome::Completed) => "Готово!".into(),
                            Ok(Outcome::Removed) => {
                                self.downloaded = 0;
                                self.total = 0;
                                "Видалено зі списку завантажень; файли збережено".into()
                            }
                            Err(error) => format!("Помилка: {error}"),
                        };
                        self.running = false;
                        self.paused = false;
                        self.action_pending = false;
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.events = None;
            self.commands = None;
        }
    }
}

impl eframe::App for TorrentApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.read_events();
        if self.running {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Простий торрент-клієнт");
            ui.add_space(16.0);

            ui.label("Торент-файл або magnet-посилання");
            ui.horizontal(|ui| {
                ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.source));
                if ui
                    .add_enabled(!self.running, egui::Button::new("Обрати файл…"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Torrent", &["torrent"])
                        .pick_file()
                {
                    self.source = path.display().to_string();
                }
            });

            ui.add_space(10.0);
            ui.label("Папка завантаження");
            ui.horizontal(|ui| {
                ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.output));
                if ui
                    .add_enabled(!self.running, egui::Button::new("Обрати папку…"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.output = path.display().to_string();
                }
            });

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.label("Ліміт швидкості (КіБ/с; 0 — без обмежень)");
                ui.add(egui::DragValue::new(&mut self.limit_kib).range(0..=4_194_303));
                if ui
                    .add_enabled(
                        self.running && !self.action_pending,
                        egui::Button::new("Застосувати"),
                    )
                    .clicked()
                {
                    self.send_command(Command::SetLimit(self.limit_bps()));
                    self.status = "Змінюю ліміт швидкості…".into();
                }
            });

            ui.add_space(12.0);
            if ui
                .add_enabled(!self.running, egui::Button::new("Завантажити"))
                .clicked()
            {
                self.start();
            }

            if self.running {
                ui.horizontal(|ui| {
                    let label = if self.paused {
                        "Відновити"
                    } else {
                        "Пауза"
                    };
                    if ui
                        .add_enabled(!self.action_pending, egui::Button::new(label))
                        .clicked()
                    {
                        let command = if self.paused {
                            Command::Resume
                        } else {
                            Command::Pause
                        };
                        self.send_command(command);
                        self.status = "Змінюю стан завантаження…".into();
                    }
                    if ui.button("Видалити зі списку").clicked() {
                        self.send_command(Command::Remove);
                        self.status = "Видаляю зі списку…".into();
                    }
                });
            }

            ui.add_space(14.0);
            ui.label(&self.status);
            if self.running || self.total > 0 {
                let progress = if self.total == 0 {
                    0.0
                } else {
                    self.downloaded as f32 / self.total as f32
                };
                ui.add(egui::ProgressBar::new(progress).show_percentage());
                if self.total > 0 {
                    ui.label(format!(
                        "{:.1} / {:.1} МБ",
                        self.downloaded as f64 / 1_000_000.0,
                        self.total as f64 / 1_000_000.0
                    ));
                }
            }
        });
    }
}

fn download(
    source: String,
    output: String,
    initial_limit: Option<NonZeroU32>,
    mut commands: UnboundedReceiver<Command>,
    sender: Sender<Event>,
) {
    let result = (|| -> Result<Outcome, Box<dyn Error>> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            let torrent = AddTorrent::from_cli_argument(&source)?;
            let output = PathBuf::from(output);
            fs::create_dir_all(&output)?;
            let session = Session::new(output).await?;
            session.ratelimits.set_download_bps(initial_limit);

            let add = session.add_torrent(torrent, None);
            tokio::pin!(add);
            let mut pause_when_ready = false;
            let handle = loop {
                tokio::select! {
                    result = &mut add => {
                        break result?.into_handle().ok_or("не вдалося почати завантаження")?;
                    }
                    Some(command) = commands.recv() => match command {
                        Command::Remove => return Ok(Outcome::Removed),
                        Command::SetLimit(limit) => {
                            session.ratelimits.set_download_bps(limit);
                            let _ = sender.send(Event::LimitApplied);
                        }
                        Command::Pause => {
                            pause_when_ready = true;
                            let _ = sender.send(Event::Paused(true));
                        }
                        Command::Resume => {
                            pause_when_ready = false;
                            let _ = sender.send(Event::Paused(false));
                        }
                    }
                }
            };
            if pause_when_ready {
                session.pause(&handle).await?;
            }

            let mut ticker = tokio::time::interval(Duration::from_millis(500));
            loop {
                tokio::select! {
                    Some(command) = commands.recv() => match command {
                        Command::Pause => {
                            session.pause(&handle).await?;
                            let _ = sender.send(Event::Paused(true));
                        }
                        Command::Resume => {
                            session.unpause(&handle).await?;
                            let _ = sender.send(Event::Paused(false));
                        }
                        Command::SetLimit(limit) => {
                            session.ratelimits.set_download_bps(limit);
                            let _ = sender.send(Event::LimitApplied);
                        }
                        Command::Remove => {
                            session.stop().await;
                            return Ok(Outcome::Removed);
                        }
                    },
                    _ = ticker.tick() => {
                        let stats = handle.stats();
                        let _ = sender.send(Event::Progress {
                            downloaded: stats.progress_bytes,
                            total: stats.total_bytes,
                        });
                        if let Some(error) = stats.error {
                            return Err(error.into());
                        }
                        if stats.finished {
                            session.stop().await;
                            return Ok(Outcome::Completed);
                        }
                    }
                }
            }
        })
    })();
    let _ = sender.send(Event::Finished(result.map_err(|error| error.to_string())));
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([620.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Простий торрент-клієнт",
        options,
        Box::new(|_context| Ok(Box::<TorrentApp>::default())),
    )
}
