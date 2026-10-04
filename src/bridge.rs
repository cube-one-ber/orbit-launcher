use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use orbit_launcher::{artwork, model::*, providers, store};
use std::{
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
    }
}
pub struct BackendRust {
    snapshot: QString,
    preferences: QString,
    message: QString,
    busy: bool,
    artwork_busy: bool,
    demo: bool,
    settings: Settings,
    library: Library,
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
            settings,
            library: Library::default(),
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
        match orbit_launcher::browser_extension::prepare(&store::config_dir(), "chrome") {
            Ok(path) => {
                let message = orbit_launcher::browser_extension::open_setup().err().unwrap_or_else(|| "Extension files are ready. In Chrome or Chromium, enable Developer mode, choose Load unpacked and select the displayed folder. Then visit Roblox while signed in.".into());
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
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (title, provider, subtitle))| Game {
        id: format!("demo:{i}"),
        title: title.into(),
        provider: provider.into(),
        subtitle: subtitle.into(),
        artwork: String::new(),
        art: Artwork {
            roblox_universe: (provider == "roblox").then_some(2440500124),
            minecraft_version: [2, 8].contains(&i).then(|| "1.21.5".into()),
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
    })
    .collect();
    Library {
        games,
        providers: [
            ("steam", "Steam", 4),
            ("lutris", "Lutris", 1),
            ("prism", "Prism Launcher", 1),
            ("modrinth", "Modrinth Launcher", 1),
            ("heroic", "Heroic Games Launcher", 1),
            ("legendary", "Legendary", 1),
            ("roblox", "Roblox", 1),
            ("battlenet", "Battle.net", 1),
            ("ubisoft", "Ubisoft Connect", 1),
            ("itch", "itch.io", 1),
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
