#!/bin/sh
# Makes the PNG icons from icons/icon.svg.
#
# Run it after changing the drawing, then commit the PNG files: the build
# only reads them, so that it needs no extra tool.
#
# Needs rsvg-convert: sudo apt install librsvg2-bin
set -eu

cd "$(dirname "$0")/../icons"

# The sizes of the freedesktop icon theme that a desktop asks for most.
# The file name is the one the Tauri bundler expects for each size.
for size in 32 64 128 256; do
    rsvg-convert --width "$size" --height "$size" icon.svg --output "${size}x${size}.png"
done
# The largest one is also the icon of the window and of the tray.
rsvg-convert --width 512 --height 512 icon.svg --output icon.png
echo "Icons written to $(pwd)"
