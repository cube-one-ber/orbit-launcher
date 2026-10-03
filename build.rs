fn main() {
    #[cfg(feature = "gui")]
    cxx_qt_build::CxxQtBuilder::new_qml_module(
        cxx_qt_build::QmlModule::new("app.orbit").qml_files([
            "qml/Main.qml",
            "qml/Theme.qml",
            "qml/AppIcon.qml",
            "qml/GameCard.qml",
            "qml/UiChecks.qml",
        ]),
    )
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .file("src/bridge.rs")
    .build();
}
