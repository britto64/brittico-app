#!/usr/bin/env bash
# Os pacotes de Linux a partir do binário pronto:
#   packaging/linux/empacotar.sh <versão> <binário> <pasta de saída>
# Gera, com nomes fixos (os links "última versão" do site dependem deles):
#   brittico-linux-x86_64.tar.gz   sempre
#   Brittico-x86_64.AppImage       se achar o appimagetool (APPIMAGETOOL=... ou no PATH)
#   brittico_amd64.deb             se houver dpkg-deb
#   brittico-x86_64.rpm            se houver rpmbuild
# O pacote do Arch sai de packaging/arch (makepkg), a partir do .tar.gz.
set -euo pipefail
VER="$1"; BIN="$2"; OUT="$(mkdir -p "$3" && cd "$3" && pwd)"
RAIZ="$(cd "$(dirname "$0")/../.." && pwd)"
P="$RAIZ/packaging"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

# Uma árvore /usr comum a todos.
arvore() { # $1 = destino (raiz do sistema de arquivos do pacote)
  install -Dm755 "$BIN" "$1/usr/bin/brittico"
  install -Dm644 "$P/linux/brittico.desktop" "$1/usr/share/applications/brittico.desktop"
  install -Dm644 "$P/linux/xyz.brittico.Brittico.metainfo.xml" "$1/usr/share/metainfo/xyz.brittico.Brittico.metainfo.xml"
  for s in 32 48 64 128 256; do
    install -Dm644 "$P/icons/brittico-$s.png" "$1/usr/share/icons/hicolor/${s}x${s}/apps/brittico.png"
  done
}

# 1. tar.gz (também é a fonte do pacote do Arch)
mkdir -p "$TMP/tar/brittico"
arvore "$TMP/tar/brittico"
cp "$RAIZ/README.md" "$TMP/tar/brittico/"
tar -C "$TMP/tar" -czf "$OUT/brittico-linux-x86_64.tar.gz" brittico
echo "ok: brittico-linux-x86_64.tar.gz"

# 2. AppImage
AIT="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -n "$AIT" ]; then
  A="$TMP/AppDir"; arvore "$A"
  install -Dm755 "$P/linux/AppRun" "$A/AppRun"
  cp "$P/linux/brittico.desktop" "$A/brittico.desktop"
  cp "$P/icons/brittico-256.png" "$A/brittico.png"
  ln -s brittico.png "$A/.DirIcon"
  # o OpenSSL 3 junto (a única biblioteca que nem toda distribuição tem nessa versão)
  for l in libssl.so.3 libcrypto.so.3; do
    f="$(ldd "$BIN" | awk -v l="$l" '$1==l {print $3}')"
    [ -n "$f" ] && install -Dm644 "$f" "$A/usr/lib/$l"
  done
  ARCH=x86_64 "$AIT" --no-appstream "$A" "$OUT/Brittico-x86_64.AppImage" >/dev/null 2>&1 \
    || ARCH=x86_64 "$AIT" --appimage-extract-and-run --no-appstream "$A" "$OUT/Brittico-x86_64.AppImage"
  echo "ok: Brittico-x86_64.AppImage"
fi

# 3. .deb
if command -v dpkg-deb >/dev/null; then
  D="$TMP/deb"; arvore "$D"
  mkdir -p "$D/DEBIAN"
  cat > "$D/DEBIAN/control" <<CTRL
Package: brittico
Version: $VER
Section: net
Priority: optional
Architecture: amd64
Depends: libc6 (>= 2.35), libssl3 | libssl3t64, libglib2.0-0 | libglib2.0-0t64, libasound2 | libasound2t64, libgl1, libxkbcommon0
Recommends: xdg-desktop-portal, pipewire
Maintainer: Brittico <noreply@brittico.xyz>
Homepage: https://brittico.xyz/app/
Description: O feed, a call e o LoL de proximidade num app só
 O Brittico como programa: o feed, a Call com voz e tela, o LivePixel e o
 LoL de proximidade.
CTRL
  dpkg-deb --root-owner-group --build "$D" "$OUT/brittico_amd64.deb" >/dev/null
  echo "ok: brittico_amd64.deb"
fi

# 4. .rpm
if command -v rpmbuild >/dev/null; then
  R="$TMP/rpm"; mkdir -p "$R"/{BUILD,RPMS,SPECS,BUILDROOT}
  arvore "$R/raiz"
  cat > "$R/SPECS/brittico.spec" <<SPEC
Name: brittico
Version: $(echo "$VER" | tr '-' '_')
Release: 1
Summary: O feed, a call e o LoL de proximidade num app só
License: Proprietary
URL: https://brittico.xyz/app/
Requires: openssl-libs >= 3, glib2, alsa-lib, libglvnd-glx, libxkbcommon
AutoReqProv: no
%description
O Brittico como programa: o feed, a Call com voz e tela, o LivePixel e o LoL de proximidade.
%install
cp -a $R/raiz/. %{buildroot}/
%files
/usr/bin/brittico
/usr/share/applications/brittico.desktop
/usr/share/metainfo/xyz.brittico.Brittico.metainfo.xml
/usr/share/icons/hicolor/*/apps/brittico.png
SPEC
  rpmbuild --define "_topdir $R" --define "_build_id_links none" -bb "$R/SPECS/brittico.spec" >/dev/null
  cp "$R"/RPMS/x86_64/*.rpm "$OUT/brittico-x86_64.rpm"
  echo "ok: brittico-x86_64.rpm"
fi
