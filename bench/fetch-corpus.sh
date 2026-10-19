#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p corpus && cd corpus
[ -d mdit-src ] || git clone -q --depth 1 https://github.com/markdown-it/markdown-it mdit-src
[ -d rust-book ] || git clone -q --depth 1 https://github.com/rust-lang/book rust-book
[ -f spec.md ] || curl -sfL https://raw.githubusercontent.com/commonmark/commonmark-spec/master/spec.txt -o spec.md

# sparse <dir> <repo> <sha> <path>...: shallow, blobless, sparse checkout pinned to one commit.
sparse() {
  local dir=$1 repo=$2 sha=$3; shift 3
  [ -d "$dir" ] && return
  git init -q "$dir"
  git -C "$dir" remote add origin "https://github.com/$repo"
  git -C "$dir" sparse-checkout set --no-cone "$@"
  git -C "$dir" fetch -q --depth 1 --filter=blob:none origin "$sha"
  git -C "$dir" checkout -q FETCH_HEAD
}

sparse node-api nodejs/node e463b487c69d1ef482a407d55410498164c0892c '/doc/api/*.md'
sparse mdn-js mdn/content a1aa79c09717761fc72c20bac94bb9c430ed487a '/files/en-us/web/javascript/reference/**'
sparse k8s-concepts kubernetes/website 079a4d63f072a54c317689c957307aaa018cfe34 '/content/en/docs/concepts/**'

# Holdouts: never used to tune. corpora.mjs takes the first N sorted paths of github-docs.
sparse github-docs github/docs dae83f7d28f3b8e10015bd6b6d8b72267cedceca '/content/**/*.md'
sparse vue-docs vuejs/docs 40aa88af0094f7bab4aaf786e55c748a6a251d88 '/src/**/*.md'
sparse rust-rfcs rust-lang/rfcs d2705ad64ba47e812a2b6b19d6f613fb4c14c5e0 '/text/*.md'
sparse tldr tldr-pages/tldr 481978fcfbac2229ccfe0dca91ce2e7d67d34ebc '/pages/common/*.md' '/pages/linux/*.md'
sparse electron-docs electron/electron 40ebdcc38d51d225b95306ee5b0289bef994b734 '/docs/**/*.md'
sparse obsidian-help obsidianmd/obsidian-help 08ef563f07b78dd589f19b1583d089c1e2d2377c '/en/**/*.md'

raw() { curl -sfL "https://raw.githubusercontent.com/$1/$2/$3" -o "$4"; }
if [ ! -d awesome ]; then
  mkdir -p awesome.tmp
  raw sindresorhus/awesome bc98e517ddca672f55f9857d714fc3ea3c3540b2 readme.md awesome.tmp/awesome.md
  raw vinta/awesome-python 3e8c86bac6ad0548b36921ba219a56592982e046 README.md awesome.tmp/awesome-python.md
  raw avelino/awesome-go 9084f305adfe8a9677a706cbac851a7b1af5fe77 README.md awesome.tmp/awesome-go.md
  raw awesome-selfhosted/awesome-selfhosted 2a5937cbdb6879e4097fa9c6b0cd810b021827e3 README.md awesome.tmp/awesome-selfhosted.md
  raw rust-unofficial/awesome-rust 65354b7306c9296f53d6c988f4def01c82b9d9ec README.md awesome.tmp/awesome-rust.md
  raw sindresorhus/awesome-nodejs b8e1c0c3879544e32dca7d381d8275ceff267e46 readme.md awesome.tmp/awesome-nodejs.md
  mv awesome.tmp awesome
fi
