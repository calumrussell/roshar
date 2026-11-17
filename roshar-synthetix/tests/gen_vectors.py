"""Generate tests/vectors.json with the official Synthetix Python SDK.

    python3 -m venv /tmp/snx-venv && /tmp/snx-venv/bin/pip install synthetix-sdk==0.7.0
    /tmp/snx-venv/bin/python tests/gen_vectors.py > tests/vectors.json

Uses a throwaway random key and makes no network calls. Every request body, signature and EIP-712
digest comes from `synthetix.signing.Signer` (SDK 0.7.0). Where the SDK reads the clock
(`generate_nonce`, used for `sign_action` nonces and the websocket auth timestamp) the generator
pins it to the vector's value. The digest is captured from the `eth_account` call the SDK makes.
Never regenerate the vectors to make a change pass.
"""

import json
import secrets
import sys
from importlib.metadata import version

import synthetix.signing as sg
from eth_account import Account

SUBACCOUNT = 1867542890123456789
EXPIRES_AFTER_MS = 60_000

key = "0x" + secrets.token_hex(32)
signer = sg.Signer(key)
delegate = Account.from_key("0x" + secrets.token_hex(32)).address

# Capture the EIP-712 digest the SDK signs.
_digests = []
_orig_sign = signer._account.sign_message


def _capture(msg):
    signed = _orig_sign(msg)
    _digests.append("0x" + bytes(signed.message_hash).hex())
    return signed


signer._account.sign_message = _capture


def pin_clock(value):
    sg.generate_nonce = lambda: value
    signer._last_nonce = 0


def compact(obj):
    return json.dumps(obj, separators=(",", ":"))


def ws_post(req_id, rest_payload):
    # synthetix/ws/manager.py WebSocketManager.post + _Connection._send_request.
    ws_params = {
        **rest_payload["params"],
        "signature": rest_payload["signature"],
        "nonce": rest_payload["nonce"],
        "expiresAfter": rest_payload["expiresAfter"],
    }
    return compact({"id": req_id, "method": "post", "params": ws_params})


vectors = []


def record(name, kind, inputs, req):
    digest = _digests.pop()
    assert not _digests
    vectors.append(
        {
            "name": name,
            "kind": kind,
            "subaccount_id": SUBACCOUNT,
            "nonce": req["nonce"],
            "expires_after": req["expiresAfter"],
            "inputs": inputs,
            "digest": digest,
            "body": compact(req),
            "ws_post": ws_post(name, req),
        }
    )


def order(**kw):
    base = {
        "symbol": "BTC-USDT",
        "side": "buy",
        "orderType": "limitGtc",
        "price": "",
        "triggerPrice": "",
        "quantity": "",
        "reduceOnly": False,
        "isTriggerMarket": False,
        "clientOrderId": "",
        "closePosition": False,
    }
    base.update(kw)
    return base


nonce = 1794780000000

place_cases = [
    (
        "place_limit_gtc_post_only_cloid",
        "na",
        [order(price="64990.5", quantity="0.0015", postOnly=True, clientOrderId="0x0123456789abcdef0123456789abcdef")],
    ),
    (
        "place_limit_ioc_reduce_only",
        "na",
        [order(side="sell", orderType="limitIoc", price="65100", quantity="0.25", reduceOnly=True)],
    ),
    (
        "place_market",
        "na",
        [order(symbol="ETH-USDT", orderType="market", quantity="1.2345")],
    ),
    (
        "place_limit_alo",
        "na",
        [order(symbol="SOL-USDT", orderType="limitAlo", price="72.03", quantity="10")],
    ),
    (
        "place_limit_gtd",
        "na",
        [order(orderType="limitGtd", price="60000", quantity="0.01", postOnly=True, expiresAt=1794780600)],
    ),
    (
        "place_batch_mixed",
        "na",
        [
            order(price="49000.00", quantity="0.05", clientOrderId="0xfedcba9876543210fedcba9876543210"),
            order(side="sell", orderType="triggerSl", triggerPrice="45000", quantity="0.15", reduceOnly=True, isTriggerMarket=True),
            order(symbol="ETH-USDT", side="sell", orderType="limitGtc", price="2800.00", quantity="2.0"),
        ],
    ),
    (
        "place_position_tpsl",
        "positionTpsl",
        [
            order(side="sell", orderType="triggerTp", price="70000", triggerPrice="70100", quantity="0", closePosition=True),
            order(side="sell", orderType="triggerSl", triggerPrice="60000", quantity="0", isTriggerMarket=True, closePosition=True),
        ],
    ),
    (
        "place_twap",
        "twap",
        [
            order(
                orderType="twap",
                quantity="1.5",
                durationSeconds=3600,
                intervalSeconds=60,
                intervalVarianceBps=1000,
                sizeVarianceBps=2000,
            )
        ],
    ),
    (
        "place_twap_minimal",
        "twap",
        [order(side="sell", orderType="twap", price="66000", quantity="0.5", durationSeconds=300)],
    ),
]

for name, grouping, orders in place_cases:
    nonce += 1
    req = signer.sign_place_orders(SUBACCOUNT, orders, grouping=grouping, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS)
    record(name, "place_orders", {"orders": orders, "grouping": grouping}, req)

nonce += 1
ids = [2122086887672799232, 1948058938469519361]
record("cancel_orders", "cancel_orders", {"order_ids": ids},
       signer.sign_cancel_orders(SUBACCOUNT, ids, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

nonce += 1
cloids = ["0x0123456789abcdef0123456789abcdef", "my-order-beta"]
record("cancel_orders_by_cloid", "cancel_orders_by_cloid", {"client_order_ids": cloids},
       signer.sign_cancel_orders_by_cloid(SUBACCOUNT, cloids, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

for name, symbols in [("cancel_all_wildcard", ["*"]), ("cancel_all_symbols", ["BTC-USDT", "ETH-USDT"])]:
    nonce += 1
    record(name, "cancel_all_orders", {"symbols": symbols},
           signer.sign_cancel_all_orders(SUBACCOUNT, symbols, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

for name, kw in [
    ("modify_price_quantity", {"price": "65010", "quantity": "0.002"}),
    ("modify_trigger_price", {"trigger_price": "61000"}),
]:
    nonce += 1
    inputs = {"order_id": 2122086887672799232, "price": kw.get("price", ""), "quantity": kw.get("quantity", ""),
              "trigger_price": kw.get("trigger_price", "")}
    record(name, "modify_order", inputs,
           signer.sign_modify_order(SUBACCOUNT, 2122086887672799232, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS, **kw))

nonce += 1
record("modify_by_cloid", "modify_order_by_cloid",
       {"client_order_id": "0x0123456789abcdef0123456789abcdef", "price": "", "quantity": "0.003", "trigger_price": ""},
       signer.sign_modify_order_by_cloid(SUBACCOUNT, "0x0123456789abcdef0123456789abcdef", quantity="0.003",
                                         nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

nonce += 1
record("update_leverage", "update_leverage", {"symbol": "BTC-USDT", "leverage": "20"},
       signer.sign_update_leverage(SUBACCOUNT, "BTC-USDT", "20", nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

for name, timeout in [("schedule_cancel_arm", 300), ("schedule_cancel_disarm", 0)]:
    nonce += 1
    record(name, "schedule_cancel", {"timeout_seconds": timeout},
           signer.sign_schedule_cancel(SUBACCOUNT, timeout, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

# Read-only queries: `sign_action` takes its nonce from the (pinned) clock.
queries = [
    ("query_open_orders", "getOpenOrders", {"symbol": "BTC-USDT", "limit": 50}),
    ("query_open_orders_cloid", "getOpenOrders", {"clientOrderId": "0x0123456789abcdef0123456789abcdef"}),
    ("query_positions", "getPositions", None),
    ("query_positions_filtered", "getPositions", {"status": "open", "symbol": "ETH-USDT", "limit": 10, "offset": 20}),
    ("query_sub_account", "getSubAccount", None),
    ("query_trades", "getTrades", {"symbol": "BTC-USDT", "orderId": "2122086887672799232", "limit": 100}),
    ("query_order_history", "getOrderHistory",
     {"status": ["filled", "cancelled"], "startTime": 1794175200000, "endTime": 1794780000000, "limit": 1000}),
    ("query_funding_payments", "getFundingPayments", {"symbol": "BTC-USDT", "startTime": 1794175200000, "limit": 100}),
    ("query_rate_limits", "getRateLimits", None),
    ("query_delegated_signers", "getDelegatedSigners", None),
]
for name, action, params in queries:
    nonce += 1
    pin_clock(nonce)
    record(name, "query", {"action": action, "params": params or {}},
           signer.sign_action(SUBACCOUNT, action, action_params=params, expires_after_ms=EXPIRES_AFTER_MS))

for name, perms, expires_at in [("add_delegate_session", ["session"], 0), ("add_delegate_expiring", ["delegate"], 1797372000000)]:
    nonce += 1
    record(name, "add_delegated_signer", {"delegate": delegate, "permissions": perms, "expires_at": expires_at},
           signer.sign_add_delegated_signer(SUBACCOUNT, delegate, perms, expires_at=expires_at, nonce=nonce,
                                            expires_after_ms=EXPIRES_AFTER_MS))

nonce += 1
record("remove_delegate", "remove_delegated_signer", {"delegate": delegate},
       signer.sign_remove_delegated_signer(SUBACCOUNT, delegate, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

nonce += 1
record("remove_all_delegates", "remove_all_delegated_signers", {},
       signer.sign_remove_all_delegated_signers(SUBACCOUNT, nonce=nonce, expires_after_ms=EXPIRES_AFTER_MS))

# Websocket auth (timestamp from the pinned clock): SDK sends ms, docs ask for seconds.
ws_auth = []
for name, ts in [("ws_auth_ms", 1794780152316), ("ws_auth_seconds", 1794780153)]:
    pin_clock(ts)
    payload, signature = signer.sign_auth(SUBACCOUNT)
    ws_auth.append(
        {
            "name": name,
            "subaccount_id": SUBACCOUNT,
            "timestamp": ts,
            "digest": _digests.pop(),
            # manager.py `_authenticate`: params.message = json.dumps(payload); compared compact.
            "message": compact(json.loads(json.dumps(payload))),
            "signature": signature,
        }
    )

json.dump(
    {
        "sdk": "synthetix-sdk " + version("synthetix-sdk"),
        "eth_account": version("eth-account"),
        "private_key": key,
        "address": signer.address,
        "vectors": vectors,
        "ws_auth": ws_auth,
    },
    sys.stdout,
    indent=1,
)
sys.stdout.write("\n")
