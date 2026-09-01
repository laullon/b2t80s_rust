#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app_dir="$project_dir/target/release/bundle/b2t80s.app"
contents_dir="$app_dir/Contents"

cd "$project_dir"
cargo build --release

mkdir -p "$contents_dir/MacOS" "$contents_dir/Resources"
cp "$project_dir/target/release/b2t80s_rust" "$contents_dir/MacOS/b2t80s_rust"
cp "$project_dir/packaging/macos/Info.plist" "$contents_dir/Info.plist"
cp "$project_dir/assets/b2t80s.icns" "$contents_dir/Resources/b2t80s.icns"

chmod +x "$contents_dir/MacOS/b2t80s_rust"
touch "$app_dir"
codesign --force --deep --sign - "$app_dir"

echo "$app_dir"
