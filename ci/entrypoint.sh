#!/bin/bash
set -euo pipefail

IBC_SRC=${IBC_SRC:-/src/ibc-rs}
BASECOIN_SRC=${BASECOIN_SRC:-/src/basecoin-rs}
BUILD_ROOT="${HOME}/build"
IBC_BUILD="${BUILD_ROOT}/ibc-rs"
BASECOIN_BUILD="${BUILD_ROOT}/basecoin-rs"
BASECOIN_BIN="${BASECOIN_BUILD}/debug/tendermint-basecoin"
HERMES_BIN="${IBC_BUILD}/release/hermes"
IBC_REPO=https://github.com/informalsystems/ibc-rs.git
IBC_COMMITISH=${IBC_COMMITISH:-master}
CHAIN_DATA="${HOME}/data"
HERMES_CONFIG="${HOME}/.hermes/config.toml"
LOG_DIR=${LOG_DIR:-/var/log/basecoin-rs}
TESTS_DIR=${TESTS_DIR:-${HOME}/tests}

if [ ! -f "${BASECOIN_SRC}/Cargo.toml" ]; then
  echo "basecoin-rs sources must be mounted into ${BASECOIN_SRC} for this script to work properly."
  exit 1
fi

if [ ! -f "${IBC_SRC}/Cargo.toml" ]; then
  echo "No ibc-rs sources detected. Cloning repo at ${IBC_COMMITISH}..."
  git clone "${IBC_REPO}" "${IBC_SRC}"
  echo "Checking out ${IBC_COMMITISH}..."
  cd "${IBC_SRC}"
  git checkout "${IBC_COMMITISH}"
  git status
  echo ""
fi

cd "${IBC_SRC}"
echo "Building Hermes..."
cargo build --release --bin hermes --target-dir "${IBC_BUILD}/"

cd "${BASECOIN_SRC}"
echo ""
echo "Building basecoin-rs..."
cargo build --target-dir "${BASECOIN_BUILD}"

echo ""
echo "Setting up chain ibc-0..."
mkdir -p "${CHAIN_DATA}"
"${HOME}/one-chain" gaiad ibc-0 "${CHAIN_DATA}" 26657 26656 6060 9090 100000000000

echo ""
echo "Configuring Hermes..."
"${HERMES_BIN}" --config "${HERMES_CONFIG}" \
    keys add --chain ibc-0 \
    --key-file "${CHAIN_DATA}/ibc-0/user_seed.json"

echo "Adding user key to basecoin-0 chain..."
"${HERMES_BIN}" --config "${HERMES_CONFIG}" \
    keys add --chain basecoin-0 \
    --key-file "${HOME}/user_seed.json"

echo ""
echo "Starting Tendermint..."
tendermint unsafe-reset-all
tendermint node > "${LOG_DIR}/tendermint.log" 2>&1 &

echo "Starting basecoin-rs..."
cd "${BASECOIN_SRC}"
"${BASECOIN_BIN}" -p 26358 -v > "${LOG_DIR}/basecoin.log" 2>&1 &

echo "Waiting for Tendermint node to be available..."
set +e
for retry in {1..4}; do
  sleep 5
  curl "http://127.0.0.1:26357/abci_info"
  CURL_STATUS=$?
  if [ ${CURL_STATUS} -eq 0 ]; then
    break
  else
    echo "curl exit code status ${CURL_STATUS} (attempt ${retry})"
  fi
done
set -e
# Will fail if we still can't reach the Tendermint node
curl "http://127.0.0.1:26357/abci_info" > /dev/null 2>&1

if [ ! -z "$@" ]; then
  cd "${HOME}"
  exec "$@"
else
  echo ""
  echo "No parameters supplied. Executing default tests from: ${TESTS_DIR}"
  for t in "${TESTS_DIR}"/*; do
    bash "$t"
  done
  echo ""
  echo "Success!"
fi
