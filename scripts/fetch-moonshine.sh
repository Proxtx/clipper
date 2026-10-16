#!/usr/bin/env bash
# Downloads the moonshine speech-to-text library and a streaming model.
# usage: scripts/fetch-moonshine.sh [language] [tiny|small|medium]
#   language: de, en, es, ja, ar, tl, vi, zh (default: de)
#   size:     small is available for de/en/es/ja, medium only for en (default: small)
set -euo pipefail

LANGUAGE=${1:-de}
SIZE=${2:-small}
LIB_VERSION=v0.1.5

cd "$(dirname "$0")/.."

case "$LANGUAGE" in
  en) MODEL_VERSION=quantized_26_08_21 ;;
  ja) MODEL_VERSION=quantized_26_08_23 ;;
  *) MODEL_VERSION=quantized_26_08_24 ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) ARCH=x86_64 ;;
  arm64 | aarch64) ARCH=arm64 ;;
  *) echo "unsupported architecture $(uname -m)"; exit 1 ;;
esac

if [ ! -f vendor/moonshine-voice/lib/libmoonshine.so ]; then
  echo "downloading libmoonshine $LIB_VERSION"
  rm -rf vendor/moonshine-voice
  mkdir -p vendor/moonshine-voice
  curl -fL "https://github.com/moonshine-ai/moonshine/releases/download/$LIB_VERSION/moonshine-voice-linux-$ARCH.tar.gz" |
    tar xz -C vendor/moonshine-voice --strip-components=1
fi

MODEL=$SIZE-streaming-$LANGUAGE
MODEL_DIR=models/$MODEL
mkdir -p "$MODEL_DIR"
for FILE in adapter.ort cross_kv.ort decoder_kv.ort encoder.ort frontend.model.ort \
  frontend.weights.ort streaming_config.json tokenizer.bin; do
  if [ ! -f "$MODEL_DIR/$FILE" ]; then
    echo "downloading $MODEL/$FILE"
    curl -fL -C - -o "$MODEL_DIR/$FILE.part" "https://download.moonshine.ai/model/$MODEL/$MODEL_VERSION/$FILE"
    mv "$MODEL_DIR/$FILE.part" "$MODEL_DIR/$FILE"
  fi
done

echo "model ready in $MODEL_DIR"
