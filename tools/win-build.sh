#!/bin/sh
# Windows release build, cross-compiled from Linux (needs mingw-w64-gcc, 7z and the
# x86_64-pc-windows-gnu Rust target). Leaves dist-win/:
#   mochi.exe                 the app
#   software-gl/mochi.exe     the same app, started when the PC has no usable OpenGL
#   software-gl/*.dll         Mesa's software OpenGL (llvmpipe), from MESA_7Z
# MESA_7Z: mesa3d-<ver>-release-mingw.7z from github.com/pal1000/mesa-dist-win/releases
# (the MSVC package has no OpenGL driver). Without it the folder is left out.
# Then MochiSetup.exe (packaging/windows/mochi.nsi) with makensis: the system's, else
# NSIS_DIR's makensis.exe (the NSIS zip from nsis.sourceforge.io) under Wine. Without
# either the installer is left out.
set -e
C=$(dirname "$(dirname "$(readlink -f "$0")")")
cd "$C"
cargo build --release --target x86_64-pc-windows-gnu
rm -rf dist-win
mkdir -p dist-win
cp target/x86_64-pc-windows-gnu/release/maia.exe dist-win/mochi.exe
cp assets/fonts/OFL.txt dist-win/FONTS-LICENSE.txt
if [ -n "$MESA_7Z" ]; then
  T=$(mktemp -d)
  7z x -o"$T" "$MESA_7Z" x64/opengl32.dll x64/libgallium_wgl.dll > /dev/null
  mkdir -p dist-win/software-gl
  cp "$T"/x64/opengl32.dll "$T"/x64/libgallium_wgl.dll dist-win/software-gl/
  cp dist-win/mochi.exe dist-win/software-gl/mochi.exe
  cp tools/MESA-LICENSE.txt dist-win/software-gl/
  rm -rf "$T"
fi
du -sh dist-win
VERSION=$(cat MOCHI_VERSION)
rm -f MochiSetup.exe
if command -v makensis > /dev/null; then
  makensis -V2 -DVERSION="$VERSION" packaging/windows/mochi.nsi
elif [ -n "$NSIS_DIR" ]; then
  WINEDEBUG=-all wine "$NSIS_DIR/makensis.exe" /V2 /DVERSION="$VERSION" "$(winepath -w packaging/windows/mochi.nsi)"
else
  echo "no makensis and no NSIS_DIR: MochiSetup.exe left out"
fi
if [ -f MochiSetup.exe ]; then du -sh MochiSetup.exe; fi
