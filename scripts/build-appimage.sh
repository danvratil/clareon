#!/usr/bin/env bash

set -ex

# use RAM disk if possible
if [ -z "$CI" ] && [ -d /dev/shm ]; then
    TEMP_BASE=/dev/shm
else
    TEMP_BASE=/tmp
fi

BUILD_DIR=$(mktemp -d -p "$TEMP_BASE" clareon-appimage-build-XXXXXX)

cleanup () {
    if [ -d "$BUILD_DIR" ]; then
        rm -rf "$BUILD_DIR"
    fi
}

[ -z "$NO_CLEANUP" ] && trap cleanup EXIT

# store repo root as variable
REPO_ROOT=$(readlink -f $(dirname "$0")/..)
OLD_CWD=$(readlink -f .)

pushd "$BUILD_DIR"

export VERSION=$(cd "$REPO_ROOT" && git describe --tags)

# standard linuxdeploy pattern
#see https://docs.appimage.org/packaging-guide/from-source/index.html for more information
cmake "$REPO_ROOT" -DCMAKE_INSTALL_PREFIX=/usr -G Ninja -DCMAKE_BUILD_TYPE=Release

ninja -k0
DESTDIR=AppDir ninja install
ln -s "usr/share/icons/hicolor/256x256/apps/cc.clareon.Clareon.png" "AppDir/.DirIcon"

curl -fLO https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
curl -fLO https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-x86_64.AppImage
curl -fLO https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage
chmod +x linuxdeploy*.AppImage

export QML_SOURCES_PATHS="${REPO_ROOT}/clareon/qml"
export NO_STRIP="true"
export QMAKE=$(which qmake6)
export APPIMAGE_EXTRACT_AND_RUN=1
./linuxdeploy-x86_64.AppImage --appdir AppDir/ --output appimage --plugin qt

mv Clareon*.AppImage "$OLD_CWD"/
