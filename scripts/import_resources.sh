#!/usr/bin/env zsh
set -euo pipefail

# Script para copiar `hulk-docs.pdf` desde la carpeta Descargas del usuario
# Ajusta SOURCE si tu PDF está en otra ruta.

SOURCE="$HOME/Downloads/hulk-docs.pdf"
DEST_DIR="$(dirname "$0")/../resources"
DEST="$DEST_DIR/hulk-docs.pdf"

mkdir -p "$DEST_DIR"
if [ -f "$SOURCE" ]; then
  cp "$SOURCE" "$DEST"
  echo "Copied $SOURCE -> $DEST"
else
  echo "Source PDF not found at $SOURCE"
  echo "Please place the PDF at $DEST or modify this script to point to your file."
  exit 1
fi
