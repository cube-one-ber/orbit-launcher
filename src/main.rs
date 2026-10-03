#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod bridge;
use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QString, QUrl};
fn main() {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        println!(
            "Orbit — your games, in one place\n\nUsage: orbit [--demo] [--light] [--smoke-test] [--screenshot PATH]\n\n--demo        Preview a sample library; no settings writes or game launches\n--light       Select light mode (saved unless --demo is supplied)\n--smoke-test  Load the UI and exit automatically after 2 seconds\n--screenshot  Save a rendered PNG to the supplied absolute path\n--ui-test     Run interaction checks (requires --demo)\n\nORBIT_CONFIG_DIR overrides the configuration directory."
        );
        return;
    }
    bridge::qobject::QQuickStyle::set_style(&QString::from("Basic"));
    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("orbit"));
        app.as_mut()
            .set_application_display_name(&QString::from("Orbit"));
        app.as_mut().set_organization_name(&QString::from("Orbit"));
        app.set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        let qml_engine: std::pin::Pin<&mut QQmlEngine> = engine.as_mut().upcast_pin();
        qml_engine
            .on_exit(|_, code| std::process::exit(code))
            .release();
        engine
            .as_mut()
            .on_object_creation_failed(|_, url| {
                eprintln!("Could not load the Orbit interface: {url}");
                std::process::exit(1);
            })
            .release();
        engine.load(&QUrl::from("qrc:/qt/qml/app/orbit/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}
