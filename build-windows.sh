#!/usr/bin/env bash
# Gera dist/brittico.exe (Windows 64 bits) daqui do Arch, com o LoL de proximidade e a voz juntos.
#
# A voz usa a libwebrtc que vem pronta, compilada com as ferramentas da Microsoft (MSVC): por isso o alvo é
# x86_64-pc-windows-msvc, montado pelo cargo-xwin (que baixa o CRT e o SDK do Windows uma vez, para ~/.cache/cargo-xwin).
# O linker é o rust-lld do próprio Rust com o nome lld-link. O .cargo/config.toml liga o crt-static (a libwebrtc é /MT).
# Precisa: rustup target add x86_64-pc-windows-msvc; cargo install cargo-xwin; clang-cl e llvm-lib (pacote clang/llvm).
set -euo pipefail
cd "$(dirname "$0")"
shim="$HOME/.cache/xwin-shim"
mkdir -p "$shim" dist
lld="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld"
ln -sf "$lld" "$shim/lld-link"
PATH="$shim:$PATH" cargo xwin build --release --target x86_64-pc-windows-msvc
cp target/x86_64-pc-windows-msvc/release/brittico.exe dist/brittico.exe
ls -la dist/brittico.exe
sha256sum dist/brittico.exe
