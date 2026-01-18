cmake_minimum_required(VERSION 3.27)

project(clareon)

find_package(ECM REQUIRED VERSION 6.0 NO_MODULE)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH})

include(GNUInstallDirs)
include(ECMInstallIcons)
include(ECMFindQmlModule)


find_package(Qt6 REQUIRED COMPONENTS Core Quick Qml)
ecm_find_qmlmodule(org.kde.kirigami REQUIRED)
ecm_find_qmlmodule(org.kde.kitemmodels REQUIRED)
ecm_find_qmlmodule(QtQuick.Controls REQUIRED)
ecm_find_qmlmodule(QtQuick.Layouts REQUIRED)
ecm_find_qmlmodule(Qt.labs.platform REQUIRED)

# Fetch and setup Corrosion
include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6 # Optionally specify a commit hash, version tag or branch here
)
FetchContent_MakeAvailable(Corrosion)

# Import targets defined in a package or workspace manifest `Cargo.toml` file
corrosion_import_crate(
    MANIFEST_PATH Cargo.toml
    CRATES clareon
)

corrosion_install(
    TARGETS clareon
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)

configure_file(
    ${CMAKE_SOURCE_DIR}/cc.clareon.desktop.cmake
    ${CMAKE_BINARY_DIR}/cc.clareon.desktop
)
install(FILES ${CMAKE_BINARY_DIR}/cc.clareon.desktop DESTINATION ${CMAKE_INSTALL_DATAROOTDIR}/applications)

ecm_install_icons(
    ICONS
        clareon/icons/16-apps-clareon.png
        clareon/icons/32-apps-clareon.png
        clareon/icons/48-apps-clareon.png
        clareon/icons/256-apps-clareon.png
        clareon/icons/sc-apps-clareon.svg
    DESTINATION ${CMAKE_INSTALL_DATAROOTDIR}/icons
)
