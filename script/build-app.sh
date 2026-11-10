#!/bin/bash
# Builds Fluent Writer.app into ./build.
# Usage: script/build-app.sh [debug|release] [--dev]
#   --dev builds "Fluent Writer Dev.app": its own bundle id, settings, drafts folder and support files,
#   so it runs side by side with the copy you write in and never touches its data.
set -euo pipefail
cd "$(dirname "$0")/.."
CONFIG="release"
VARIANT="standard"
for arg in "$@"; do
  case "$arg" in
    debug|release) CONFIG="$arg" ;;
    --dev) VARIANT="dev" ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done
if [ "$VARIANT" = dev ]; then
  NAME="Fluent Writer Dev"; BUNDLE_ID="com.sunkenintime.fluentwriter.dev"
else
  NAME="Fluent Writer"; BUNDLE_ID="com.sunkenintime.fluentwriter"
fi
swift build -c "$CONFIG" --product FluentWriter
swift build -c "$CONFIG" --product fluent-writer-mcp
BIN="$(swift build -c "$CONFIG" --show-bin-path)"
APP="build/$NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN/FluentWriter" "$APP/Contents/MacOS/FluentWriter"
cp "$BIN/fluent-writer-mcp" "$APP/Contents/MacOS/fluent-writer-mcp"
cp -R "$BIN/FluentWriter_FluentWriter.bundle" "$APP/Contents/Resources/"
if [ -d bridge/claude ]; then
  mkdir -p "$APP/Contents/Resources/bridge"
  rsync -a --exclude node_modules/.cache bridge/claude "$APP/Contents/Resources/bridge/"
fi
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>FluentWriter</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.2.0</string>
  <key>CFBundleVersion</key><string>2</string>
  <key>FWVariant</key><string>$VARIANT</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Markdown Draft</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>net.daringfireball.markdown</string><string>public.plain-text</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP" >/dev/null 2>&1 || true
echo "$APP"
