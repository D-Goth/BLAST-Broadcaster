#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_ROOT="${HOME}/.local/share/icons/hicolor"
NAME="blast-broadcaster"

refresh() {
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "${APP_DIR}" >/dev/null 2>&1 || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "${ICON_ROOT}" >/dev/null 2>&1 || true
    command -v kbuildsycoca6 >/dev/null 2>&1 && kbuildsycoca6 >/dev/null 2>&1 || true
    command -v kbuildsycoca5 >/dev/null 2>&1 && kbuildsycoca5 >/dev/null 2>&1 || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -f "${BIN_DIR}/${NAME}" "${BIN_DIR}/blast" "${APP_DIR}/${NAME}.desktop"
    for size in 32 128 256 512; do
        rm -f "${ICON_ROOT}/${size}x${size}/apps/${NAME}.png"
    done
    refresh
    echo "BLAST Broadcaster désinstallé."
    exit 0
fi

cd "${ROOT}"
echo "Compilation (release)…"
cargo build --release -p blast-app -p blast-cli

mkdir -p "${BIN_DIR}" "${APP_DIR}"
install -m 755 "target/release/${NAME}" "${BIN_DIR}/${NAME}"
install -m 755 "target/release/blast" "${BIN_DIR}/blast"

ICONS="app/src-tauri/icons"
install -Dm 644 "${ICONS}/32x32.png" "${ICON_ROOT}/32x32/apps/${NAME}.png"
install -Dm 644 "${ICONS}/128x128.png" "${ICON_ROOT}/128x128/apps/${NAME}.png"
install -Dm 644 "${ICONS}/128x128@2x.png" "${ICON_ROOT}/256x256/apps/${NAME}.png"
install -Dm 644 "${ICONS}/512x512.png" "${ICON_ROOT}/512x512/apps/${NAME}.png"

sed "s|^Exec=.*|Exec=${BIN_DIR}/${NAME} %F|" "packaging/linux/${NAME}.desktop" > "${APP_DIR}/${NAME}.desktop"
chmod 644 "${APP_DIR}/${NAME}.desktop"

refresh

echo
echo "BLAST Broadcaster est installé :"
echo "  menu des applications : BLAST Broadcaster (catégorie Multimédia)"
echo "  terminal              : ${NAME}   (outil de test : blast)"
case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *) echo "  ajoute ${BIN_DIR} à ton PATH pour lancer depuis le terminal" ;;
esac
