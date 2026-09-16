#!/bin/sh
# Fills src-tauri/models/ with the two models the app ships. Every file is
# pinned by sha256; a mismatch is a failure, not a warning. Idempotent: a file
# already present and matching is kept.
#   scripts/fetch-models.sh
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out=$root/src-tauri/models
hf=https://huggingface.co

if command -v sha256sum >/dev/null 2>&1; then
  sum() { sha256sum "$1" | cut -c1-64; }
else
  sum() { shasum -a 256 "$1" | cut -c1-64; }
fi

# fetch <dest dir> <repo> <path in repo> <name on disk> <sha256>
fetch() {
  dest=$1/$4
  if [ -f "$dest" ] && [ "$(sum "$dest")" = "$5" ]; then
    return 0
  fi
  mkdir -p "$1"
  printf 'fetching %s/%s\n' "$2" "$3"
  curl -fsSL --retry 5 --retry-all-errors --retry-delay 2 \
    -o "$dest.part" "$hf/$2/resolve/main/$3"
  got=$(sum "$dest.part")
  if [ "$got" != "$5" ]; then
    rm -f "$dest.part"
    printf 'checksum mismatch for %s: %s\n' "$3" "$got" >&2
    exit 1
  fi
  mv "$dest.part" "$dest"
}

# multilingual-e5-small, int8 (MIT; the int8 export is Xenova's).
e=Xenova/multilingual-e5-small
d=$out/embedder
fetch "$d" $e onnx/model_int8.onnx      model.onnx              4d24e2bc01a447951524466ef533e52944bf48509e6552810bcee1a2711cb02c
fetch "$d" $e tokenizer.json            tokenizer.json          0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39
fetch "$d" $e config.json               config.json             cb99455288675345e1a4f411438d5d0adbba5fbd3a67ea4fb03c015433b996c1
fetch "$d" $e special_tokens_map.json   special_tokens_map.json d05497f1da52c5e09554c0cd874037a083e1dc1b9cfd48034d1c717f1afc07a7
fetch "$d" $e tokenizer_config.json     tokenizer_config.json   a1d6bc8734a6f635dc158508bef000f8e2e5a759c7d92f984b2c86e5ff53425b

# mmarco-mMiniLMv2-L12-H384-v1, int8 (Apache 2.0). The quint8_avx2 export is
# used on every platform so a universal macOS bundle needs one resource set.
r=cross-encoder/mmarco-mMiniLMv2-L12-H384-v1
d=$out/reranker
fetch "$d" $r onnx/model_quint8_avx2.onnx model.onnx              6c2513767fb63d008a4377bef7a7a3555433d9436342bb53e35a3a72ffc52d4b
fetch "$d" $r tokenizer.json              tokenizer.json          62c24cdc13d4c9952d63718d6c9fa4c287974249e16b7ade6d5a85e7bbb75626
fetch "$d" $r config.json                 config.json             cc2cfe51aa3fd759d21d21acf5dfd6994aa67a3c9210636d22e143699d336c77
fetch "$d" $r special_tokens_map.json     special_tokens_map.json 378eb3bf733eb16e65792d7e3fda5b8a4631387ca04d2015199c4d4f22ae554d
fetch "$d" $r tokenizer_config.json       tokenizer_config.json   e7fbfbfa6347b4e414c1cee50d142e2c2f9a895dad68b068ae83a8b564c3837e

printf 'models are in %s\n' "$out"
