#!/usr/bin/env bash
#
# Integration tests for the Lightning Payments REST API.
# Requires: curl, jq
# Assumes: server, PostgreSQL, and LND nodes are already running.
#
# Usage:
#   ./test_api.sh                              # default http://localhost:8080
#   BASE_URL=http://localhost:3000 ./test_api.sh

set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:3000}"

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

PASSED=0
FAILED=0
TOTAL=0

# --- Helpers ---

fail() {
    TOTAL=$((TOTAL + 1))
    FAILED=$((FAILED + 1))
    echo -e "  ${RED}FAIL${NC} $1"
    if [[ -n "${2:-}" ]]; then
        echo -e "       ${RED}$2${NC}"
    fi
}

pass() {
    TOTAL=$((TOTAL + 1))
    PASSED=$((PASSED + 1))
    echo -e "  ${GREEN}PASS${NC} $1"
}

# Make an HTTP request. Sets globals: HTTP_STATUS, HTTP_BODY
# Usage: http GET /api/balance
#        http POST /api/invoice '{"amount_sats":1000}'
http() {
    local method="$1"
    local path="$2"
    local body="${3:-}"
    local url="${BASE_URL}${path}"

    local tmp
    tmp=$(mktemp)

    if [[ -n "$body" ]]; then
        HTTP_STATUS=$(curl -s -o "$tmp" -w '%{http_code}' \
            -X "$method" \
            -H 'Content-Type: application/json' \
            -d "$body" \
            "$url")
    else
        HTTP_STATUS=$(curl -s -o "$tmp" -w '%{http_code}' \
            -X "$method" \
            "$url")
    fi

    HTTP_BODY=$(cat "$tmp")
    rm -f "$tmp"
}

assert_status() {
    local expected="$1"
    local label="$2"
    if [[ "$HTTP_STATUS" == "$expected" ]]; then
        pass "$label (HTTP $HTTP_STATUS)"
    else
        fail "$label" "Expected HTTP $expected, got $HTTP_STATUS. Body: $HTTP_BODY"
    fi
}

assert_json_field() {
    local field="$1"
    local label="$2"
    local value
    value=$(echo "$HTTP_BODY" | jq -r ".$field // empty" 2>/dev/null)
    if [[ -n "$value" ]]; then
        pass "$label (.$field present)"
    else
        fail "$label" "Missing JSON field .$field in: $HTTP_BODY"
    fi
}

assert_json_field_equals() {
    local field="$1"
    local expected="$2"
    local label="$3"
    local value
    value=$(echo "$HTTP_BODY" | jq -r ".$field // empty" 2>/dev/null)
    if [[ "$value" == "$expected" ]]; then
        pass "$label (.$field == \"$expected\")"
    else
        fail "$label" "Expected .$field=\"$expected\", got \"$value\""
    fi
}

assert_json_array() {
    local label="$1"
    local is_array
    is_array=$(echo "$HTTP_BODY" | jq 'if type == "array" then "yes" else "no" end' -r 2>/dev/null)
    if [[ "$is_array" == "yes" ]]; then
        pass "$label (is array)"
    else
        fail "$label" "Expected JSON array, got: $HTTP_BODY"
    fi
}

json_array_length() {
    echo "$HTTP_BODY" | jq 'length' 2>/dev/null
}

json_field() {
    echo "$HTTP_BODY" | jq -r ".$1 // empty" 2>/dev/null
}

# --- Preflight ---

echo -e "${CYAN}=== Lightning Payments API Integration Tests ===${NC}"
echo -e "Target: ${YELLOW}${BASE_URL}${NC}"
echo ""

for cmd in curl jq; do
    if ! command -v "$cmd" &>/dev/null; then
        echo -e "${RED}Error: '$cmd' is required but not installed.${NC}"
        exit 1
    fi
done

# Quick connectivity check
if ! curl -s -o /dev/null -w '' --connect-timeout 5 "${BASE_URL}/api/balance" 2>/dev/null; then
    echo -e "${RED}Error: Cannot reach ${BASE_URL}. Is the server running?${NC}"
    exit 1
fi

# ==================================================================
# 1. GET /api/balance — initial state
# ==================================================================
echo -e "${CYAN}--- 1. GET /api/balance (initial) ---${NC}"
http GET /api/balance
assert_status 200 "Balance endpoint returns 200"
assert_json_field "received_sats" "Balance has received_sats"
assert_json_field "paid_sats" "Balance has paid_sats"
assert_json_field "total_balance" "Balance has total_balance"
assert_json_field "last_updated" "Balance has last_updated"
INITIAL_RECEIVED=$(json_field received_sats)
INITIAL_PAID=$(json_field paid_sats)

# ==================================================================
# 2. GET /api/transactions — initial state
# ==================================================================
echo -e "${CYAN}--- 2. GET /api/transactions (initial) ---${NC}"
http GET /api/transactions
assert_status 200 "Transactions endpoint returns 200"
assert_json_array "Transactions returns array"
INITIAL_TX_COUNT=$(json_array_length)

# ==================================================================
# 3. POST /api/invoice — create invoice
# ==================================================================
echo -e "${CYAN}--- 3. POST /api/invoice (create) ---${NC}"
http POST /api/invoice '{"amount_sats": 1000, "description": "integration test"}'
assert_status 201 "Create invoice returns 201"
assert_json_field "payment_request" "Invoice has payment_request"
assert_json_field "payment_hash" "Invoice has payment_hash"
assert_json_field "amount_sats" "Invoice has amount_sats"
assert_json_field_equals "amount_sats" "1000" "Invoice amount matches request"
INVOICE_PR=$(json_field payment_request)
INVOICE_HASH=$(json_field payment_hash)

# ==================================================================
# 4. POST /api/invoice — validation (amount <= 0)
# ==================================================================
echo -e "${CYAN}--- 4. POST /api/invoice (validation) ---${NC}"
http POST /api/invoice '{"amount_sats": 0}'
assert_status 400 "Invoice with amount 0 returns 400"
assert_json_field "error" "Error response has error field"

http POST /api/invoice '{"amount_sats": -5}'
assert_status 400 "Invoice with negative amount returns 400"

# ==================================================================
# 5. GET /api/invoice/{hash} — lookup pending invoice
# ==================================================================
echo -e "${CYAN}--- 5. GET /api/invoice/{hash} ---${NC}"

# The background LND subscription may need a moment to persist the invoice
echo -e "  ${YELLOW}(waiting 2s for background invoice sync)${NC}"
sleep 2

http GET "/api/invoice/${INVOICE_HASH}"
assert_status 200 "Get invoice by hash returns 200"
assert_json_field_equals "payment_hash" "$INVOICE_HASH" "Invoice hash matches"
INVOICE_STATUS=$(json_field status)
if [[ "$INVOICE_STATUS" == "pending" || "$INVOICE_STATUS" == "succeeded" ]]; then
    pass "Invoice status is valid ($INVOICE_STATUS)"
else
    fail "Invoice status" "Expected pending or succeeded, got $INVOICE_STATUS"
fi

# ==================================================================
# 6. POST /api/payment — pay the invoice
# ==================================================================
echo -e "${CYAN}--- 6. POST /api/payment ---${NC}"
http POST /api/payment "{\"payment_request\": \"${INVOICE_PR}\"}"
assert_status 200 "Pay invoice returns 200"
assert_json_field "payment_hash" "Payment has payment_hash"
assert_json_field "preimage" "Payment has preimage"
assert_json_field "amount_sats" "Payment has amount_sats"
assert_json_field_equals "amount_sats" "1000" "Payment amount matches invoice"
PAYMENT_HASH=$(json_field payment_hash)

# ==================================================================
# 7. POST /api/payment — validation (empty request)
# ==================================================================
echo -e "${CYAN}--- 7. POST /api/payment (validation) ---${NC}"
http POST /api/payment '{"payment_request": ""}'
assert_status 400 "Payment with empty request returns 400"
assert_json_field "error" "Error response has error field"

# ==================================================================
# 8. POST /api/payment — duplicate payment guard
# ==================================================================
echo -e "${CYAN}--- 8. POST /api/payment (duplicate) ---${NC}"
http POST /api/payment "{\"payment_request\": \"${INVOICE_PR}\"}"
assert_status 400 "Duplicate payment returns 400"
assert_json_field "error" "Duplicate error has error field"

# ==================================================================
# 9. GET /api/payment/{hash} — verify succeeded
# ==================================================================
echo -e "${CYAN}--- 9. GET /api/payment/{hash} ---${NC}"
http GET "/api/payment/${PAYMENT_HASH}"
assert_status 200 "Get payment by hash returns 200"
assert_json_field_equals "status" "succeeded" "Payment status is succeeded"
assert_json_field "preimage" "Payment has preimage"

# ==================================================================
# 10. GET /api/invoice/{hash} — verify settled
# ==================================================================
echo -e "${CYAN}--- 10. GET /api/invoice/{hash} (post-payment) ---${NC}"

# Give the background task a moment to process the settlement
echo -e "  ${YELLOW}(waiting 2s for invoice settlement sync)${NC}"
sleep 2

http GET "/api/invoice/${INVOICE_HASH}"
assert_status 200 "Get settled invoice returns 200"
assert_json_field_equals "status" "succeeded" "Invoice status is succeeded (settled)"

# ==================================================================
# 11. GET /api/transactions — verify count increased
# ==================================================================
echo -e "${CYAN}--- 11. GET /api/transactions (post-payment) ---${NC}"
http GET /api/transactions
assert_status 200 "Transactions endpoint still returns 200"
NEW_TX_COUNT=$(json_array_length)
if [[ "$NEW_TX_COUNT" -gt "$INITIAL_TX_COUNT" ]]; then
    pass "Transaction count increased ($INITIAL_TX_COUNT -> $NEW_TX_COUNT)"
else
    fail "Transaction count" "Expected count > $INITIAL_TX_COUNT, got $NEW_TX_COUNT"
fi

# ==================================================================
# 12. GET /api/balance — verify balance changed
# ==================================================================
echo -e "${CYAN}--- 12. GET /api/balance (post-payment) ---${NC}"
http GET /api/balance
assert_status 200 "Balance endpoint still returns 200"
NEW_RECEIVED=$(json_field received_sats)
NEW_PAID=$(json_field paid_sats)
if [[ "$NEW_RECEIVED" -gt "$INITIAL_RECEIVED" || "$NEW_PAID" -gt "$INITIAL_PAID" ]]; then
    pass "Balance changed (received: $INITIAL_RECEIVED->$NEW_RECEIVED, paid: $INITIAL_PAID->$NEW_PAID)"
else
    fail "Balance unchanged" "received: $INITIAL_RECEIVED->$NEW_RECEIVED, paid: $INITIAL_PAID->$NEW_PAID"
fi

# ==================================================================
# 13. GET /api/transactions?limit=1&offset=0 — pagination
# ==================================================================
echo -e "${CYAN}--- 13. GET /api/transactions (pagination) ---${NC}"
http GET "/api/transactions?limit=1&offset=0"
assert_status 200 "Paginated transactions returns 200"
assert_json_array "Paginated result is array"
PAGINATED_COUNT=$(json_array_length)
if [[ "$PAGINATED_COUNT" -eq 1 ]]; then
    pass "Pagination limit=1 returns exactly 1 result"
else
    fail "Pagination" "Expected 1 result, got $PAGINATED_COUNT"
fi

# ==================================================================
# 14. GET /api/invoice/nonexistent — 404
# ==================================================================
echo -e "${CYAN}--- 14. GET /api/invoice/nonexistent ---${NC}"
http GET /api/invoice/0000000000000000000000000000000000000000000000000000000000000000
assert_status 404 "Nonexistent invoice returns 404"
assert_json_field "error" "404 response has error field"

# ==================================================================
# 15. GET /api/payment/nonexistent — 404
# ==================================================================
echo -e "${CYAN}--- 15. GET /api/payment/nonexistent ---${NC}"
http GET /api/payment/0000000000000000000000000000000000000000000000000000000000000000
assert_status 404 "Nonexistent payment returns 404"
assert_json_field "error" "404 response has error field"

# ==================================================================
# Summary
# ==================================================================
echo ""
echo -e "${CYAN}===============================${NC}"
echo -e "  Total:  ${TOTAL}"
echo -e "  ${GREEN}Passed: ${PASSED}${NC}"
if [[ "$FAILED" -gt 0 ]]; then
    echo -e "  ${RED}Failed: ${FAILED}${NC}"
    echo -e "${CYAN}===============================${NC}"
    exit 1
else
    echo -e "  ${RED}Failed: ${FAILED}${NC}"
    echo -e "${CYAN}===============================${NC}"
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
