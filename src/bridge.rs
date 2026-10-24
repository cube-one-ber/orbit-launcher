use crate::{
    artwork,
    instances::{
        ContentKind,
        session::{Job, Outcome, Session},
    },
    model::*,
    providers, store,
};
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("QtQuickControls2/QQuickStyle");
        type QQuickStyle;
        #[Self = "QQuickStyle"]
        #[rust_name = "set_style"]
        fn setStyle(style: &QString);

    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, snapshot)]
        #[qproperty(QString, preferences)]
        #[qproperty(QString, message)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, artwork_busy)]
        #[qproperty(bool, demo)]
        #[qproperty(QString, instance_view)]
        #[qproperty(bool, instance_busy)]
        type Backend = super::BackendRust;
        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);
        #[qinvokable]
        fn check_roblox_report(self: Pin<&mut Self>);
        #[qinvokable]
        fn poll(self: Pin<&mut Self>);
        #[qinvokable]
        fn launch(self: Pin<&mut Self>, id: &QString);
        #[qinvokable]
        fn favorite(self: Pin<&mut Self>, id: &QString);
        #[qinvokable]
        fn configure(self: Pin<&mut Self>, json: &QString) -> bool;
        #[qinvokable]
        fn add_game(self: Pin<&mut Self>, json: &QString) -> bool;
        #[qinvokable]
        fn remove_game(self: Pin<&mut Self>, id: &QString);
        #[qinvokable]
        fn set_artwork(self: Pin<&mut Self>, id: &QString, path: &QString) -> bool;
        #[qinvokable]
        fn config_path(&self) -> QString;
        #[qinvokable]
        fn local_path(&self, url: &QString) -> QString;
        #[qinvokable]
        fn prepare_config(self: Pin<&mut Self>) -> QString;
        #[qinvokable]
        fn prepare_roblox_extension(self: Pin<&mut Self>) -> QString;
        #[qinvokable]
        fn open_instance(self: Pin<&mut Self>, id: &QString) -> bool;
        #[qinvokable]
        fn close_instance(self: Pin<&mut Self>);
        #[qinvokable]
        fn inspect_content(self: Pin<&mut Self>, kind: &QString);
        #[qinvokable]
        fn search_content(self: Pin<&mut Self>, kind: &QString, query: &QString, offset: i32);
        #[qinvokable]
        fn install_content(self: Pin<&mut Self>, kind: &QString, project: &QString);
        #[qinvokable]
        fn update_content(self: Pin<&mut Self>, kind: &QString, file_name: &QString);
        #[qinvokable]
        fn toggle_content(self: Pin<&mut Self>, kind: &QString, file_name: &QString) -> bool;
        #[qinvokable]
        fn remove_content(self: Pin<&mut Self>, kind: &QString, file_name: &QString) -> bool;
        #[qinvokable]
        fn add_content_file(self: Pin<&mut Self>, kind: &QString, path: &QString) -> bool;
        #[qinvokable]
        fn save_instance_settings(self: Pin<&mut Self>, json: &QString) -> bool;
        #[qinvokable]
        fn instance_folder(self: Pin<&mut Self>, kind: &QString) -> QString;
    }
}
pub struct BackendRust {
    snapshot: QString,
    preferences: QString,
    message: QString,
    busy: bool,
    artwork_busy: bool,
    demo: bool,
    instance_view: QString,
    instance_busy: bool,
    settings: Settings,
    library: Library,
    session: Option<Session>,
    jobs: VecDeque<Job>,
    job_receiver: Option<Receiver<(u64, Outcome)>>,
    session_generation: u64,
    running_instance: Option<String>,
    receiver: Option<Receiver<Library>>,
    artwork_receiver: Option<Receiver<artwork::ArtworkUpdate>>,
    artwork_cancel: Option<Arc<AtomicBool>>,
    rescan: bool,
    load_error: Option<String>,
    roblox_signature: Option<(std::path::PathBuf, SystemTime, u64)>,
}
impl Default for BackendRust {
    fn default() -> Self {
        let demo = std::env::args().any(|arg| arg == "--demo");
        let stored = if demo {
            Ok(Settings::default())
        } else {
            store::load(&store::config_dir())
        };
        let (settings, error) = match stored {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(e)),
        };
        Self {
            snapshot: QString::from("{\"games\":[],\"providers\":[]}"),
            preferences: QString::from(serde_json::to_string(&settings).unwrap().as_str()),
            message: QString::from(error.as_deref().unwrap_or("")),
            busy: false,
            artwork_busy: false,
            demo,
            instance_view: QString::from("{}"),
            instance_busy: false,
            settings,
            library: Library::default(),
            session: None,
            jobs: VecDeque::new(),
            job_receiver: None,
            session_generation: 0,
            running_instance: None,
            receiver: None,
            artwork_receiver: None,
            artwork_cancel: None,
            rescan: false,
            load_error: error,
            roblox_signature: None,
        }
    }
}
impl Drop for BackendRust {
    fn drop(&mut self) {
        if let Some(cancel) = &self.artwork_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl qobject::Backend {
    pub fn prepare_roblox_extension(mut self: Pin<&mut Self>) -> QString {
        if *self.demo() {
            self.set_message(QString::from("Preview mode · Extension setup is disabled."));
            return QString::default();
        }
        match crate::browser_extension::prepare(&store::config_dir(), "chrome") {
            Ok(path) => {
                let message = crate::browser_extension::open_setup().err().unwrap_or_else(|| "Extension files are ready. In Chrome or Chromium, enable Developer mode, choose Load unpacked and select the displayed folder. Then visit Roblox while signed in.".into());
                self.as_mut().set_message(QString::from(message.as_str()));
                QString::from(path.to_string_lossy().as_ref())
            }
            Err(error) => {
                self.set_message(QString::from(error.as_str()));
                QString::default()
            }
        }
    }
    fn publish(mut self: Pin<&mut Self>) {
        let snapshot = serde_json::to_string(&self.rust().library).unwrap();
        let settings = serde_json::to_string(&self.rust().settings).unwrap();
        self.as_mut().set_snapshot(QString::from(snapshot.as_str()));
        self.as_mut()
            .set_preferences(QString::from(settings.as_str()));
    }
    fn commit(mut self: Pin<&mut Self>, settings: Settings) -> bool {
        // Never replace a malformed settings file with defaults silently.
        if let Some(error) = self.rust().load_error.clone() {
            self.as_mut().set_message(QString::from(
                format!("Settings are read-only until the invalid file is repaired: {error}")
                    .as_str(),
            ));
            return false;
        }
        if !*self.demo()
            && let Err(e) = store::save(&store::config_dir(), &settings)
        {
            self.as_mut().set_message(QString::from(
                format!("Could not save settings: {e}").as_str(),
            ));
            return false;
        }
        self.as_mut().rust_mut().settings = settings;
        self.publish();
        true
    }
    pub fn refresh(mut self: Pin<&mut Self>) {
        if *self.busy() {
            self.as_mut().rust_mut().rescan = true;
            return;
        }
        self.as_mut().cancel_artwork();
        if *self.demo() {
            self.as_mut().rust_mut().library = demo_library();
            let settings = self.rust().settings.clone();
            self.as_mut()
                .rust_mut()
                .library
                .games
                .extend(settings.custom_games.clone());
            providers::decorate(&mut self.as_mut().rust_mut().library, &settings);
            artwork::apply_cached(
                &mut self.as_mut().rust_mut().library,
                &settings,
                store::cache_dir().join("artwork"),
            );
            self.publish();
            return;
        }
        self.as_mut().set_busy(true);
        let settings = self.rust().settings.clone();
        let config = settings.sources.get("roblox").cloned().unwrap_or_default();
        self.as_mut().rust_mut().roblox_signature = providers::roblox_report_signature(&config);
        let (tx, rx) = mpsc::channel();
        self.as_mut().rust_mut().receiver = Some(rx);
        std::thread::spawn(move || {
            let mut library = providers::discover(&settings, &store::config_dir());
            artwork::apply_cached(&mut library, &settings, store::cache_dir().join("artwork"));
            let _ = tx.send(library);
        });
    }
    pub fn check_roblox_report(mut self: Pin<&mut Self>) {
        if *self.demo() || *self.busy() || *self.artwork_busy() {
            return;
        }
        let config = self
            .rust()
            .settings
            .sources
            .get("roblox")
            .cloned()
            .unwrap_or_default();
        if !config.enabled {
            return;
        }
        let signature = providers::roblox_report_signature(&config);
        if signature != self.rust().roblox_signature {
            self.as_mut().refresh();
        }
    }
    pub fn poll(mut self: Pin<&mut Self>) {
        self.as_mut().poll_instance();
        let result = self.rust().receiver.as_ref().map(|rx| rx.try_recv());
        match result {
            Some(Ok(mut library)) => {
                providers::decorate(&mut library, &self.rust().settings);
                self.as_mut().rust_mut().library = library;
                self.as_mut().rust_mut().receiver = None;
                self.as_mut().set_busy(false);
                self.as_mut().publish();
                if self.rust().rescan {
                    self.as_mut().rust_mut().rescan = false;
                    self.as_mut().refresh();
                } else if self.rust().settings.online_artwork {
                    self.as_mut().start_artwork();
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.as_mut().rust_mut().receiver = None;
                self.as_mut().set_busy(false);
                self.as_mut().set_message(QString::from(
                    "Library scan stopped unexpectedly. Try refreshing.",
                ));
            }
            _ => (),
        }
        let mut changed = false;
        for _ in 0..32 {
            let result = self
                .rust()
                .artwork_receiver
                .as_ref()
                .map(|rx| rx.try_recv());
            match result {
                Some(Ok(update)) => {
                    if let Some(game) = self
                        .as_mut()
                        .rust_mut()
                        .library
                        .games
                        .iter_mut()
                        .find(|g| g.id == update.id)
                    {
                        game.artwork = update.artwork;
                        game.art = update.art;
                        changed = true;
                    }
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.as_mut().rust_mut().artwork_receiver = None;
                    self.as_mut().rust_mut().artwork_cancel = None;
                    self.as_mut().set_artwork_busy(false);
                    break;
                }
                _ => break,
            }
        }
        if changed {
            artwork::share_covers(&mut self.as_mut().rust_mut().library.games);
            self.publish();
        }
    }
    fn cancel_artwork(mut self: Pin<&mut Self>) {
        if let Some(cancel) = self.as_mut().rust_mut().artwork_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.as_mut().rust_mut().artwork_receiver = None;
        self.set_artwork_busy(false);
    }
    fn start_artwork(mut self: Pin<&mut Self>) {
        let games = self.rust().library.games.clone();
        if games.is_empty() {
            return;
        }
        let settings = self.rust().settings.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().artwork_receiver = Some(rx);
        self.as_mut().rust_mut().artwork_cancel = Some(cancel.clone());
        self.as_mut().set_artwork_busy(true);
        std::thread::spawn(move || {
            artwork::download(
                games,
                settings,
                store::cache_dir().join("artwork"),
                cancel,
                tx,
            )
        });
    }
    pub fn favorite(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        let mut settings = self.rust().settings.clone();
        if settings.favorites.contains(&id) {
            settings.favorites.retain(|x| x != &id);
        } else {
            settings.favorites.push(id);
        }
        if self.as_mut().commit(settings.clone()) {
            providers::decorate(&mut self.as_mut().rust_mut().library, &settings);
            self.publish();
        }
    }
    pub fn launch(mut self: Pin<&mut Self>, id: &QString) {
        if self.rust().running_instance.as_deref() == Some(id.to_string().as_str()) {
            self.set_message(QString::from(
                "Wait for the instance task to finish before launching.",
            ));
            return;
        }
        if *self.demo() {
            self.set_message(QString::from("Preview mode · Launching is disabled. Run Orbit without --demo to use your library."));
            return;
        }
        let Some(game) = self
            .rust()
            .library
            .games
            .iter()
            .find(|g| g.id == id.to_string())
            .cloned()
        else {
            return;
        };
        match providers::launch(&game) {
            Ok(()) => {
                if !game.launch_notice.is_empty() {
                    self.set_message(QString::from(game.launch_notice.as_str()));
                    return;
                }
                let mut settings = self.rust().settings.clone();
                settings.played.insert(
                    game.id.clone(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );
                if self.as_mut().commit(settings.clone()) {
                    providers::decorate(&mut self.as_mut().rust_mut().library, &settings);
                    self.as_mut().publish();
                    self.set_message(QString::from(
                        format!("Launch request sent for {}", game.title).as_str(),
                    ));
                }
            }
            Err(e) => self.set_message(QString::from(e.as_str())),
        }
    }
    pub fn configure(mut self: Pin<&mut Self>, json: &QString) -> bool {
        // Only expose appearance and sources here; favorite/history/custom edits have separate APIs.
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json.to_string());
        let result = parsed.map_err(|e| e.to_string()).and_then(|value| {
            let mut settings = self.rust().settings.clone();
            if let Some(v) = value.get("theme") {
                settings.theme = serde_json::from_value(v.clone())
                    .map_err(|_| "Theme must be dark or light".to_string())?;
            }
            if let Some(v) = value.get("online_artwork") {
                settings.online_artwork = v
                    .as_bool()
                    .ok_or("Artwork download preference must be true or false")?;
            }
            if let Some(v) = value.get("minecraft_artwork") {
                settings.minecraft_artwork = serde_json::from_value(v.clone())
                    .map_err(|_| "Minecraft artwork must be updates or modpacks")?;
            }
            if let Some(v) = value.get("density").and_then(|v| v.as_str())
                && ["comfortable", "compact"].contains(&v)
            {
                settings.density = v.into();
            }
            if let Some(v) = value.get("view").and_then(|v| v.as_str())
                && ["grid", "list"].contains(&v)
            {
                settings.view = v.into();
            }
            if let Some(v) = value.get("sources") {
                settings.sources = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
                if settings
                    .sources
                    .values()
                    .any(|source| source.paths.iter().any(|p| !p.is_absolute()))
                {
                    return Err(
                        "Library paths must be absolute (for example C:/Games or /mnt/games)."
                            .into(),
                    );
                }
            }
            Ok(settings)
        });
        match result {
            Ok(s) => {
                let sources_changed = s.sources != self.rust().settings.sources
                    || s.online_artwork != self.rust().settings.online_artwork
                    || s.minecraft_artwork != self.rust().settings.minecraft_artwork;
                if self.as_mut().commit(s) {
                    if sources_changed {
                        self.as_mut().refresh();
                    }
                    return true;
                }
            }
            Err(e) => self.set_message(QString::from(format!("Invalid settings: {e}").as_str())),
        }
        false
    }
    pub fn add_game(mut self: Pin<&mut Self>, json: &QString) -> bool {
        let result = serde_json::from_str::<serde_json::Value>(&json.to_string())
            .map_err(|e| e.to_string())
            .and_then(|v| {
                let title = v["title"].as_str().unwrap_or("").trim();
                let command: Vec<String> =
                    serde_json::from_value(v["command"].clone()).map_err(|e| e.to_string())?;
                if title.is_empty() || command.first().is_none_or(|s| s.trim().is_empty()) {
                    return Err("A title and executable are required.".into());
                }
                let artwork = v["artwork"].as_str().unwrap_or("");
                let artwork = if artwork.is_empty() {
                    String::new()
                } else {
                    providers::file_url(std::path::Path::new(artwork))
                };
                let directory = v["directory"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from);
                if directory.as_ref().is_some_and(|p| !p.is_dir()) {
                    return Err("Working directory does not exist.".into());
                }
                Ok(Game {
                    id: format!(
                        "custom:{}",
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos()
                    ),
                    title: title.into(),
                    provider: "custom".into(),
                    subtitle: "Custom game".into(),
                    artwork,
                    art: Artwork {
                        icon: providers::first_art([
                            std::path::Path::new(&command[0]).with_extension("ico")
                        ]),
                        ..Default::default()
                    },
                    launch_notice: String::new(),
                    command,
                    directory,
                    environment: Default::default(),
                    launch_uri: None,
                    favorite: false,
                    source_rank: 0,
                    last_played: 0,
                    instance: None,
                })
            });
        match result {
            Ok(game) => {
                let mut settings = self.rust().settings.clone();
                settings.custom_games.push(game);
                if self.as_mut().commit(settings) {
                    self.as_mut().refresh();
                    self.set_message(QString::from("Custom game added to your library."));
                    return true;
                }
            }
            Err(e) => self.set_message(QString::from(e.as_str())),
        }
        false
    }
    pub fn remove_game(mut self: Pin<&mut Self>, id: &QString) {
        let mut settings = self.rust().settings.clone();
        settings.custom_games.retain(|g| g.id != id.to_string());
        settings.artwork_overrides.remove(&id.to_string());
        settings.favorites.retain(|g| g != &id.to_string());
        settings.played.remove(&id.to_string());
        if self.as_mut().commit(settings) {
            self.refresh();
        }
    }
    pub fn set_artwork(mut self: Pin<&mut Self>, id: &QString, path: &QString) -> bool {
        let id = id.to_string();
        if !self.rust().library.games.iter().any(|g| g.id == id) {
            return false;
        }
        let path = path.to_string();
        if !path.is_empty()
            && (!std::path::Path::new(&path).is_absolute()
                || !std::path::Path::new(&path).is_file())
        {
            self.set_message(QString::from(
                "Choose an existing local image for the cover.",
            ));
            return false;
        }
        let mut settings = self.rust().settings.clone();
        if path.is_empty() {
            settings.artwork_overrides.remove(&id);
        } else {
            settings.artwork_overrides.insert(id, path);
        }
        if !self.as_mut().commit(settings) {
            return false;
        }
        self.refresh();
        true
    }
    pub fn local_path(&self, url: &QString) -> QString {
        QString::from(
            url::Url::parse(&url.to_string())
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
                .as_str(),
        )
    }
    pub fn config_path(&self) -> QString {
        QString::from(store::config_dir().to_string_lossy().as_ref())
    }
    pub fn prepare_config(mut self: Pin<&mut Self>) -> QString {
        if *self.demo() {
            self.set_message(QString::from(
                "Run Orbit without --demo to manage provider files.",
            ));
            return QString::default();
        }
        let dir = store::config_dir();
        if let Err(error) = std::fs::create_dir_all(dir.join("providers")) {
            self.as_mut().set_message(QString::from(
                format!("Could not open configuration: {error}").as_str(),
            ));
            return QString::default();
        }
        QString::from(
            url::Url::from_directory_path(dir)
                .map(|u| u.to_string())
                .unwrap_or_default()
                .as_str(),
        )
    }
}
/// Instance manager: one open session, network jobs on worker threads, file changes inline.
impl qobject::Backend {
    fn publish_instance(mut self: Pin<&mut Self>) {
        let view = self
            .rust()
            .session
            .as_ref()
            .map(|s| s.view().to_string())
            .unwrap_or_else(|| "{}".into());
        self.as_mut()
            .set_instance_view(QString::from(view.as_str()));
    }
    pub fn open_instance(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        let demo = *self.demo();
        let session = self
            .rust()
            .library
            .games
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| Session::new(g, demo));
        let opened = session.is_some();
        self.as_mut().rust_mut().session = session;
        self.as_mut().rust_mut().session_generation += 1;
        self.as_mut().rust_mut().jobs.clear();
        self.publish_instance();
        opened
    }
    pub fn close_instance(mut self: Pin<&mut Self>) {
        // A running job finishes in the background; its outcome is discarded.
        self.as_mut().rust_mut().session = None;
        self.as_mut().rust_mut().session_generation += 1;
        self.as_mut().rust_mut().jobs.clear();
        self.publish_instance();
    }
    fn queue_job(mut self: Pin<&mut Self>, job: Job) {
        if self.rust().session.is_none() {
            return;
        }
        if *self.demo() {
            if let Some(session) = self.as_mut().rust_mut().session.as_mut() {
                session.demo_job(&job);
            }
            self.publish_instance();
            return;
        }
        let jobs = &mut self.as_mut().rust_mut().jobs;
        // A newer search replaces one that has not started yet.
        if matches!(job, Job::Search { .. }) {
            jobs.retain(|j| !matches!(j, Job::Search { .. }));
        }
        if !jobs.contains(&job) {
            jobs.push_back(job);
        }
        self.as_mut().start_job();
        self.publish_instance();
    }
    fn start_job(mut self: Pin<&mut Self>) {
        if self.rust().job_receiver.is_some() {
            return;
        }
        let icons = self
            .rust()
            .settings
            .online_artwork
            .then(|| store::cache_dir().join("artwork"));
        let prepared = {
            let mut rust = self.as_mut().rust_mut();
            let rust = &mut *rust;
            match (rust.jobs.pop_front(), rust.session.as_mut()) {
                (Some(job), Some(session)) => {
                    session.started(&job);
                    let installed = session.installed_projects(job.kind());
                    Some((
                        job,
                        session.instance.clone(),
                        session.game_id.clone(),
                        installed,
                    ))
                }
                _ => {
                    rust.jobs.clear();
                    None
                }
            }
        };
        let Some((job, instance, game_id, installed)) = prepared else {
            self.as_mut().set_instance_busy(false);
            return;
        };
        let (tx, rx) = mpsc::channel();
        let generation = self.rust().session_generation;
        self.as_mut().rust_mut().running_instance = Some(game_id.clone());
        self.as_mut().rust_mut().job_receiver = Some(rx);
        self.as_mut().set_instance_busy(true);
        std::thread::spawn(move || {
            let result = crate::instances::session::run(&job, &instance, &installed, icons);
            let _ = tx.send((
                generation,
                Outcome {
                    game_id,
                    job,
                    result,
                },
            ));
        });
    }
    fn poll_instance(mut self: Pin<&mut Self>) {
        let result = self.rust().job_receiver.as_ref().map(|rx| rx.try_recv());
        let follow_up = match result {
            Some(Ok((generation, outcome))) if generation == self.rust().session_generation => self
                .as_mut()
                .rust_mut()
                .session
                .as_mut()
                .and_then(|s| s.apply(outcome)),
            Some(Ok(_)) => None,
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                if let Some(session) = self.as_mut().rust_mut().session.as_mut() {
                    session.status.clear();
                    session.error = "The Modrinth task stopped unexpectedly. Try again.".into();
                }
                None
            }
            _ => return,
        };
        self.as_mut().rust_mut().job_receiver = None;
        self.as_mut().rust_mut().running_instance = None;
        if let Some(job) = follow_up
            && !self.rust().jobs.contains(&job)
        {
            self.as_mut().rust_mut().jobs.push_back(job);
        }
        self.as_mut().start_job();
        self.publish_instance();
    }
    pub fn inspect_content(self: Pin<&mut Self>, kind: &QString) {
        if let Some(kind) = ContentKind::parse(&kind.to_string()) {
            self.queue_job(Job::Inspect(kind));
        }
    }
    pub fn search_content(self: Pin<&mut Self>, kind: &QString, query: &QString, offset: i32) {
        if let Some(kind) = ContentKind::parse(&kind.to_string()) {
            self.queue_job(Job::Search {
                kind,
                query: query.to_string(),
                offset: offset.max(0) as u64,
            });
        }
    }
    pub fn install_content(self: Pin<&mut Self>, kind: &QString, project: &QString) {
        if let Some(kind) = ContentKind::parse(&kind.to_string()) {
            self.queue_job(Job::Install {
                kind,
                project: project.to_string(),
            });
        }
    }
    pub fn update_content(self: Pin<&mut Self>, kind: &QString, file_name: &QString) {
        if let Some(kind) = ContentKind::parse(&kind.to_string()) {
            let file_name = Some(file_name.to_string()).filter(|f| !f.is_empty());
            self.queue_job(Job::Update { kind, file_name });
        }
    }
    /// Runs a file change unless a Modrinth task is using the instance.
    fn change_files(
        mut self: Pin<&mut Self>,
        kind: &QString,
        change: impl FnOnce(&mut Session, ContentKind) -> bool,
    ) -> bool {
        let busy = *self.instance_busy();
        let kind = ContentKind::parse(&kind.to_string());
        let changed = match (self.as_mut().rust_mut().session.as_mut(), kind) {
            (Some(session), Some(_)) if busy => {
                session.status.clear();
                session.error = "Wait for the current Modrinth task to finish.".into();
                false
            }
            (Some(session), Some(kind)) => change(session, kind),
            _ => return false,
        };
        self.publish_instance();
        changed
    }
    pub fn toggle_content(self: Pin<&mut Self>, kind: &QString, file_name: &QString) -> bool {
        let file_name = file_name.to_string();
        self.change_files(kind, |session, kind| session.toggle(kind, &file_name))
    }
    pub fn remove_content(self: Pin<&mut Self>, kind: &QString, file_name: &QString) -> bool {
        let file_name = file_name.to_string();
        self.change_files(kind, |session, kind| session.remove(kind, &file_name))
    }
    pub fn add_content_file(mut self: Pin<&mut Self>, kind: &QString, path: &QString) -> bool {
        let path = std::path::PathBuf::from(path.to_string());
        let added = self
            .as_mut()
            .change_files(kind, |session, kind| session.add_file(kind, &path));
        // Identify the new file if this list already shows Modrinth details.
        if let Some(kind) = ContentKind::parse(&kind.to_string())
            && added
            && self
                .rust()
                .session
                .as_ref()
                .is_some_and(|s| s.was_inspected(kind))
        {
            self.queue_job(Job::Inspect(kind));
        }
        added
    }
    pub fn save_instance_settings(mut self: Pin<&mut Self>, json: &QString) -> bool {
        if *self.instance_busy() {
            if let Some(session) = self.as_mut().rust_mut().session.as_mut() {
                session.error = "Wait for the current instance task to finish.".into();
            }
            self.publish_instance();
            return false;
        }
        let json = json.to_string();
        let saved = self
            .as_mut()
            .rust_mut()
            .session
            .as_mut()
            .is_some_and(|s| s.save_settings(&json));
        self.as_mut().publish_instance();
        if saved {
            // Show the new instance name in the library.
            self.refresh();
        }
        saved
    }
    pub fn instance_folder(mut self: Pin<&mut Self>, kind: &QString) -> QString {
        let kind = ContentKind::parse(&kind.to_string());
        let url = match self.as_mut().rust_mut().session.as_mut() {
            Some(session) => match session.folder(kind) {
                Ok(path) => url::Url::from_directory_path(path)
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                Err(error) => {
                    session.status.clear();
                    session.error = error;
                    String::new()
                }
            },
            None => return QString::default(),
        };
        self.publish_instance();
        QString::from(url.as_str())
    }
}
fn demo_library() -> Library {
    let games = [
        ("The Outer Worlds", "legendary", "A new frontier awaits"),
        (
            "Hollow Knight",
            "steam",
            "Descend into the forgotten kingdom",
        ),
        ("Minecraft", "prism", "Minecraft 1.21.5 · Spring to Life"),
        ("Celeste", "steam", "Find your way to the summit"),
        ("Hades", "lutris", "Defy the god of the dead"),
        ("Stardew Valley", "steam", "Make yourself at home"),
        ("No Man’s Sky", "steam", "An infinite universe to explore"),
        ("Disco Elysium", "heroic", "Every choice leaves a mark"),
        (
            "Fabulously Optimized",
            "modrinth",
            "Minecraft 1.21.5 · Modrinth Launcher",
        ),
        ("DOORS", "roblox", "#1 · 4h 20m last week · @Demo_Player"),
        ("Diablo IV", "battlenet", "Installed · Battle.net"),
        (
            "Assassin’s Creed Odyssey",
            "ubisoft",
            "Installed · Ubisoft Connect",
        ),
        ("A Short Hike", "itch", "Installed · itch.io"),
        ("SuperTuxKart", "desktop", "Installed · Desktop game"),
        ("RetroArch", "steam", "Non-Steam game · Steam"),
        ("MultiMC Survival", "multimc", "Minecraft 1.21.5 · MultiMC"),
        ("PolyMC Creative", "polymc", "Minecraft 1.21.5 · PolyMC"),
        (
            "ATLauncher Adventure",
            "atlauncher",
            "Minecraft 1.21.5 · ATLauncher",
        ),
        ("Amazon Adventure", "nile", "Amazon Games · Nile"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (title, provider, subtitle))| Game {
        id: if i == 14 {
            "steam:shortcut:demo".into()
        } else {
            format!("demo:{i}")
        },
        title: title.into(),
        provider: provider.into(),
        subtitle: subtitle.into(),
        artwork: String::new(),
        art: Artwork {
            roblox_universe: (provider == "roblox").then_some(2440500124),
            minecraft_version: [2, 8, 15, 16, 17].contains(&i).then(|| "1.21.5".into()),
            modrinth_project: (i == 8).then(|| "1KVo5zza".into()),
            remote: artwork::steam_urls(match i {
                0 => "578650",
                1 => "367520",
                3 => "504230",
                4 => "1145360",
                5 => "413150",
                6 => "275850",
                7 => "632470",
                10 => "2344520",
                11 => "812140",
                12 => "1055540",
                _ => "",
            }),
            ..Default::default()
        },
        launch_notice: String::new(),
        command: vec![],
        environment: Default::default(),
        launch_uri: None,
        directory: None,
        favorite: false,
        source_rank: if provider == "roblox" { 1 } else { 0 },
        last_played: if i == 0 { 100 } else { 0 },
        // Sample instances open the manager with preview content only.
        instance: matches!(provider, "prism" | "modrinth" | "multimc" | "polymc").then(|| {
            MinecraftInstance {
                launcher: provider.into(),
                minecraft_version: Some("1.21.5".into()),
                loader: (provider != "polymc").then(|| "fabric".into()),
                loader_version: (provider == "prism").then(|| "0.16.14".into()),
                modpack: (provider == "modrinth").then(|| "a Modrinth modpack".into()),
                ..Default::default()
            }
        }),
    })
    .collect();
    Library {
        games,
        providers: [
            ("steam", "Steam", 5),
            ("lutris", "Lutris", 1),
            ("prism", "Prism Launcher", 1),
            ("multimc", "MultiMC", 1),
            ("polymc", "PolyMC", 1),
            ("atlauncher", "ATLauncher", 1),
            ("nile", "Nile (Amazon Games)", 1),
            ("modrinth", "Modrinth Launcher", 1),
            ("heroic", "Heroic Games Launcher", 1),
            ("legendary", "Legendary", 1),
            ("roblox", "Roblox", 1),
            ("battlenet", "Battle.net", 1),
            ("ubisoft", "Ubisoft Connect", 1),
            ("itch", "itch.io", 1),
            ("desktop", "Desktop games", 1),
        ]
        .map(|(id, name, count)| ProviderStatus {
            id: id.into(),
            name: name.into(),
            count,
            enabled: true,
            available: true,
            note: String::new(),
            errors: vec![],
        })
        .into(),
    }
}
