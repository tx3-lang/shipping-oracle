#!/usr/bin/env bash
# Smoke test for the running shipping-oracle backend.
# Hits /health and a handful of Shippo demo tracking numbers, prints parsed JSON.
#
# Usage:
#   ./scripts/smoke.sh                       # uses http://localhost:3000
#   BASE_URL=http://host:port ./scripts/smoke.sh

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:3000}"
CARRIER="${CARRIER:-shippo}"

# Shippo exposes deterministic demo tracking numbers — see
# https://docs.goshippo.com/docs/tracking/tracking/
TRACKING_NUMBERS=(
  "SHIPPO_PRE_TRANSIT"
  "SHIPPO_TRANSIT"
  "SHIPPO_DELIVERED"
)

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required (brew install jq)" >&2
  exit 1
fi

echo "→ GET ${BASE_URL}/health"
if ! curl -fsS --max-time 5 "${BASE_URL}/health" | jq .; then
  echo "error: health check failed — is the oracle running on ${BASE_URL}?" >&2
  exit 1
fi

for tn in "${TRACKING_NUMBERS[@]}"; do
  echo
  echo "→ GET ${BASE_URL}/v1/shipment?carrier=${CARRIER}&tracking_number=${tn}"
  curl -fsS --max-time 15 \
    --get "${BASE_URL}/v1/shipment" \
    --data-urlencode "carrier=${CARRIER}" \
    --data-urlencode "tracking_number=${tn}" \
    | jq '{
        status: .data.status,
        timestamp: .data.timestamp,
        carrier_hash: .data.carrier_hash,
        tracking_number_hash: .data.tracking_number_hash,
        plaintext: .plaintext,
        public_key: .public_key,
        signature: .signature,
        cbor_hex: .cbor_hex
      }'
done

echo
echo "✓ smoke test ok"
