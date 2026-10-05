use super::{
    installed::{self, Installed},
    *,
};

pub struct Heroic;
impl Provider for Heroic {
    fn id(&self) -> &str {
        "heroic"
    }
    fn name(&self) -> &str {
        "Heroic Games Launcher"
    }
    fn note(&self) -> &str {
        "Installed Epic, GOG and Amazon games. Play requests --no-gui; Heroic retains Wine/Proton, settings and accounts. Login/error prompts may still appear. AppImages and portable installations can use a command override."
    }
    fn discover(&self, config: &SourceConfig) -> (Vec<Game>, Vec<String>) {
        let platform = crate::platform::Platform::current();
        let mut games = vec![];
        let mut errors = vec![];
        for root in roots(config, platform.heroic_roots()) {
            if !root.is_dir() {
                errors.push(format!(
                    "{}: Choose Heroic's configuration folder",
                    root.display()
                ));
                continue;
            }
            let root = fs::canonicalize(&root).unwrap_or(root);
            let key = url::Url::from_directory_path(&root)
                .map(|u| u.to_string())
                .unwrap_or_default();
            for runner in ["legendary", "gog", "nile"] {
                let records = match runner {
                    "legendary" => {
                        installed::epic_games(&root.join("legendaryConfig/legendary"), &mut errors)
                    }
                    "gog" => gog_games(&root, &mut errors),
                    _ => {
                        installed::amazon_games(&root.join("nile_config/nile"), &mut errors, false)
                    }
                };
                let library = installed::read_json(
                    &root.join(format!("store_cache/{runner}_library.json")),
                    &mut errors,
                );
                let library = library
                    .as_ref()
                    .and_then(|v| v[if runner == "gog" { "games" } else { "library" }].as_array());
                for mut installed in records {
                    if let Some(info) = library
                        .into_iter()
                        .flatten()
                        .find(|m| m["app_name"].as_str() == Some(&installed.id))
                    {
                        if info["install"]["is_dlc"].as_bool() == Some(true) {
                            continue;
                        }
                        if let Some(title) = info["title"].as_str().filter(|s| !s.trim().is_empty())
                        {
                            installed.title = title.into();
                        }
                        installed::merge_art(&mut installed.art, info);
                    }
                    let mut uri = url::Url::parse("heroic://launch").expect("constant URL");
                    uri.query_pairs_mut()
                        .append_pair("appName", &installed.id)
                        .append_pair("runner", runner)
                        // Recent Heroic builds also respect this when already running.
                        .append_pair("gui", "false");
                    let mut command = launcher(
                        config,
                        is_flatpak(&root),
                        &platform.heroic_command(),
                        "com.heroicgameslauncher.hgl",
                    );
                    if !command.iter().any(|arg| arg == "--no-gui") {
                        command.push("--no-gui".into());
                    }
                    command.push(uri.to_string());
                    let store = match runner {
                        "legendary" => "Epic Games",
                        "gog" => "GOG",
                        _ => "Amazon Games",
                    };
                    games.push(Game {
                        id: format!("heroic:{key}:{runner}:{}", installed.id),
                        title: installed.title,
                        provider: self.id().into(),
                        subtitle: format!("{store} · Heroic"),
                        artwork: crate::artwork::local_cover(&installed.path),
                        art: installed.art,
                        launch_notice: String::new(),
                        command,
                        environment: Default::default(),
                        launch_uri: None,
                        directory: None,
                        favorite: false,
                        source_rank: 0,
                        last_played: 0,
                    });
                }
            }
        }
        (games, errors)
    }
}

fn gog_games(root: &Path, errors: &mut Vec<String>) -> Vec<Installed> {
    let file = root.join("gog_store/installed.json");
    let Some(value) = installed::read_json(&file, errors) else {
        return vec![];
    };
    let Some(records) = value["installed"].as_array() else {
        errors.push(format!("{}: Expected an installed array", file.display()));
        return vec![];
    };
    records
        .iter()
        .filter_map(|info| {
            let id = info["appName"].as_str().unwrap_or("");
            if !installed::valid_id(id) {
                errors.push(format!("{}: Invalid GOG appName", file.display()));
                return None;
            }
            if info["is_dlc"].as_bool() == Some(true) {
                return None;
            }
            let path = installed::installed_path(
                info["install_path"].as_str().unwrap_or(""),
                &file,
                errors,
            )?;
            if path.join(".gogdl-resume").exists() {
                return None;
            }
            Some(Installed {
                id: id.into(),
                title: installed::title_or_folder(info["title"].as_str(), &path),
                art: Artwork {
                    icon: crate::artwork::local_icon(&path),
                    gog_product: Some(id.into()),
                    ..Default::default()
                },
                path,
            })
        })
        .collect()
}
