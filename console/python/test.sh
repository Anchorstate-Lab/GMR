#!/bin/sh
set -eu
cd "$(dirname "$0")"
python3 -m venv .venv-test
. .venv-test/bin/activate
pip -q install maturin
env -u CONDA_PREFIX VIRTUAL_ENV="$PWD/.venv-test" maturin develop -q
python test/verbs.py
