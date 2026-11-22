#!/bin/sh
# Runs the official MCP conformance suite against dogu.
#   cargo build --release && sh tests/conformance.sh
# Needs Node. The suite starts its own test servers on localhost.
M="$(cd "$(dirname "$0")/.." && pwd)/target/release/dogu"
PASS=0; FAIL=0
for s in initialize tools_call request-metadata http-standard-headers http-custom-headers \
  http-invalid-tool-headers json-schema-ref-no-deref json-schema-2020-12-preservation \
  auth/metadata-default auth/metadata-var1 auth/metadata-var2 auth/metadata-var3 auth/basic-cimd \
  auth/scope-from-www-authenticate auth/scope-from-scopes-supported auth/scope-omitted-when-undefined \
  auth/scope-step-up auth/scope-retry-limit auth/token-endpoint-auth-basic auth/token-endpoint-auth-post \
  auth/token-endpoint-auth-none auth/pre-registration auth/2025-03-26-oauth-metadata-backcompat \
  auth/2025-03-26-oauth-endpoint-fallback auth/resource-mismatch auth/offline-access-scope \
  auth/offline-access-not-supported auth/authorization-server-migration auth/iss-supported \
  auth/iss-not-advertised auth/iss-supported-missing auth/iss-wrong-issuer auth/iss-unexpected \
  auth/iss-normalized auth/metadata-issuer-mismatch; do
  if npx -y @modelcontextprotocol/conformance@alpha client --command "$M conformance" --scenario "$s" --force 2>&1 | grep -q "OVERALL: PASSED"; then
    PASS=$((PASS + 1)); echo "pass  $s"
  else
    FAIL=$((FAIL + 1)); echo "FAIL  $s"
  fi
done
echo "$PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
