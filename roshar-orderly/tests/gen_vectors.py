"""Generate Orderly signing vectors with the official Python SDK.

SDK: github.com/OrderlyNetwork/orderly-evm-connector-python (see SDK_COMMIT). Run from a throwaway
venv, never from the repo:

    uv venv -p 3.12 /tmp/orderly-venv
    git clone https://github.com/OrderlyNetwork/orderly-evm-connector-python /tmp/orderly-sdk
    VIRTUAL_ENV=/tmp/orderly-venv uv pip install -r /tmp/orderly-sdk/requirements.txt
    VIRTUAL_ENV=/tmp/orderly-venv uv pip install --no-deps /tmp/orderly-sdk
    /tmp/orderly-venv/bin/python roshar-orderly/tests/gen_vectors.py > roshar-orderly/tests/vectors.json

Keys are fresh random throwaway keys. Nothing touches the network: the SDK's `_dispatch_request`
is replaced with a recorder, and its clock (`get_timestamp`) with fixed values.
"""

import base64
import json
import secrets
import subprocess
import sys

import base58
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)
from eth_abi import encode as abi_encode
from eth_account import Account
from eth_utils import keccak

import orderly_evm_connector
import orderly_evm_connector.lib.utils as sdk_utils
import orderly_evm_connector.rest._account as sdk_account
from orderly_evm_connector.rest import Rest

SDK_DIR = "/tmp/orderly-sdk"
BASE_URL = "https://api.orderly.org"


def sdk_commit():
    try:
        return subprocess.check_output(
            ["git", "-C", SDK_DIR, "rev-parse", "HEAD"], text=True
        ).strip()
    except Exception:
        return "unknown"


# ── throwaway keys ──────────────────────────────────────────────────────────

seed = secrets.token_bytes(32)
priv = Ed25519PrivateKey.from_private_bytes(seed)
pub = priv.public_key().public_bytes_raw()
ORDERLY_SECRET = "ed25519:" + base58.b58encode(seed).decode()
ORDERLY_KEY = "ed25519:" + base58.b58encode(pub).decode()

wallet = Account.create()
WALLET_SECRET = wallet.key.hex()
if WALLET_SECRET.startswith("0x"):
    WALLET_SECRET = WALLET_SECRET[2:]
WALLET_ADDRESS = wallet.address  # EIP-55 checksummed

BROKER_ID = "woofi_pro"


def account_id(address, broker_id):
    """Account id formula from https://orderly.network/docs/build-on-omnichain/user-flows/accounts
    (`keccak256(abi.encode(address, keccak256(brokerId)))`), computed with the SDK's own
    eth-abi / eth-utils dependencies."""
    return "0x" + keccak(
        abi_encode(["address", "bytes32"], [address, keccak(text=broker_id)])
    ).hex().removeprefix("0x")


ACCOUNT_ID = account_id(WALLET_ADDRESS, BROKER_ID)

client = Rest(
    orderly_key=ORDERLY_KEY,
    orderly_secret=ORDERLY_SECRET,
    wallet_secret=WALLET_SECRET,
    orderly_account_id=ACCOUNT_ID,
    orderly_api_url=BASE_URL,
)


# ── recorder ────────────────────────────────────────────────────────────────

class FakeResponse:
    status_code = 200
    text = "{}"
    headers = {}

    def json(self):
        return {}


captured = {}


def dispatch(http_method, params):
    captured["method"] = http_method
    captured["url"] = params["url"]
    captured["params"] = params["params"]
    captured["headers"] = dict(client.session.headers)
    return FakeResponse()


client._dispatch_request = dispatch


def fix_clock(ts):
    sdk_utils.get_timestamp = lambda: ts
    sdk_account.get_timestamp = lambda: ts


def num(x):
    """The JSON text Python emits for a number (what the SDK sends)."""
    return json.dumps(x)


def record(name, ts, call, inputs):
    fix_clock(ts)
    captured.clear()
    call()
    method = captured["method"]
    assert captured["url"].startswith(BASE_URL)
    path = captured["url"][len(BASE_URL):]
    params = captured["params"]
    # requests' `json=` serialises with `json.dumps` defaults, the same text the SDK signs.
    body = json.dumps(params) if method in ("POST", "PUT") and params else None
    h = captured["headers"]
    message = f"{ts}{method}{path}{body or ''}"
    sig = base64.b64decode(h["orderly-signature"])
    Ed25519PublicKey.from_public_bytes(pub).verify(sig, message.encode())
    assert h["orderly-timestamp"] == str(ts)
    assert h["orderly-key"] == ORDERLY_KEY
    assert h["orderly-account-id"] == ACCOUNT_ID
    return {
        "name": name,
        "timestamp": ts,
        "method": method,
        "path": path,
        "body": body,
        "message": message,
        "signature": h["orderly-signature"],
        "inputs": inputs,
    }


requests_out = []
ts = 1794780000000


def add(name, call, inputs):
    global ts
    ts += 1111
    requests_out.append(record(name, ts, call, inputs))


# ── orders ──────────────────────────────────────────────────────────────────

add(
    "create_limit_cloid",
    lambda: client.create_order(
        symbol="PERP_BTC_USDC",
        order_type="LIMIT",
        side="BUY",
        client_order_id="roshar-0001",
        order_price=65000.5,
        order_quantity=0.0015,
        reduce_only=False,
    ),
    {
        "symbol": "PERP_BTC_USDC",
        "order_type": "LIMIT",
        "side": "BUY",
        "client_order_id": "roshar-0001",
        "order_price": num(65000.5),
        "order_quantity": num(0.0015),
        "reduce_only": False,
    },
)
add(
    "create_post_only",
    lambda: client.create_order(
        symbol="PERP_ETH_USDC",
        order_type="POST_ONLY",
        side="SELL",
        order_price=1850.25,
        order_quantity=1.2,
    ),
    {
        "symbol": "PERP_ETH_USDC",
        "order_type": "POST_ONLY",
        "side": "SELL",
        "order_price": num(1850.25),
        "order_quantity": num(1.2),
    },
)
add(
    "create_ioc_reduce_only",
    lambda: client.create_order(
        symbol="PERP_SOL_USDC",
        order_type="IOC",
        side="SELL",
        client_order_id="ioc-7",
        order_price=120,
        order_quantity=3,
        reduce_only=True,
    ),
    {
        "symbol": "PERP_SOL_USDC",
        "order_type": "IOC",
        "side": "SELL",
        "client_order_id": "ioc-7",
        "order_price": num(120),
        "order_quantity": num(3),
        "reduce_only": True,
    },
)
add(
    "create_market_amount_tag",
    lambda: client.create_order(
        symbol="PERP_BTC_USDC",
        order_type="MARKET",
        side="BUY",
        order_amount=250.75,
        visible_quantity=0,
        order_tag="enum:woofi_pro:GRID",
    ),
    {
        "symbol": "PERP_BTC_USDC",
        "order_type": "MARKET",
        "side": "BUY",
        "order_amount": num(250.75),
        "visible_quantity": num(0),
        "order_tag": "enum:woofi_pro:GRID",
    },
)
add(
    "create_tiny_quantity",
    lambda: client.create_order(
        symbol="PERP_BTC_USDC",
        order_type="FOK",
        side="BUY",
        order_price=64999.9,
        order_quantity=0.00001,
    ),
    {
        "symbol": "PERP_BTC_USDC",
        "order_type": "FOK",
        "side": "BUY",
        "order_price": num(64999.9),
        "order_quantity": num(0.00001),
    },
)

# Documented create-order fields the SDK's `create_order` does not take, sent through the SDK's own
# signing path (`_sign_request`) in the crate's key order.
extended = {
    "symbol": "PERP_ETH_USDC",
    "order_type": "POST_ONLY",
    "side": "BUY",
    "client_order_id": "ext-1",
    "order_price": 1700.1,
    "order_quantity": 0.5,
    "reduce_only": False,
    "post_only_adjust": True,
    "margin_mode": "ISOLATED",
}
add(
    "create_extended_fields",
    lambda: client._sign_request("POST", "/v1/order", payload=dict(extended)),
    {
        **{k: v for k, v in extended.items()},
        "order_price": num(1700.1),
        "order_quantity": num(0.5),
    },
)
market_slippage = {
    "symbol": "PERP_BTC_USDC",
    "order_type": "MARKET",
    "side": "SELL",
    "order_quantity": 0.01,
    "slippage": 0.005,
    "margin_mode": "CROSS",
}
add(
    "create_market_slippage",
    lambda: client._sign_request("POST", "/v1/order", payload=dict(market_slippage)),
    {**market_slippage, "order_quantity": num(0.01), "slippage": num(0.005)},
)
bid_level = {
    "symbol": "PERP_BTC_USDC",
    "order_type": "BID",
    "side": "BUY",
    "order_quantity": 0.002,
    "level": 2,
}
add(
    "create_bid_level",
    lambda: client._sign_request("POST", "/v1/order", payload=dict(bid_level)),
    {**bid_level, "order_quantity": num(0.002)},
)

batch_orders = [
    {
        "symbol": "PERP_BTC_USDC",
        "order_type": "LIMIT",
        "side": "BUY",
        "client_order_id": "b-1",
        "order_price": 64000,
        "order_quantity": 0.001,
    },
    {
        "symbol": "PERP_BTC_USDC",
        "order_type": "POST_ONLY",
        "side": "SELL",
        "client_order_id": "b-2",
        "order_price": 66000.1,
        "order_quantity": 0.001,
        "reduce_only": True,
    },
]
add(
    "batch_create",
    lambda: client.batch_create_order([dict(o) for o in batch_orders]),
    {
        "orders": [
            {
                **o,
                "order_price": num(o["order_price"]),
                "order_quantity": num(o["order_quantity"]),
            }
            for o in batch_orders
        ]
    },
)

add(
    "edit_order",
    lambda: client.edit_order(
        order_id="292820969",
        symbol="PERP_BTC_USDC",
        order_type="LIMIT",
        side="BUY",
        order_price=64950.1,
        order_quantity=0.002,
    ),
    {
        "order_id": 292820969,
        "symbol": "PERP_BTC_USDC",
        "order_type": "LIMIT",
        "side": "BUY",
        "order_price": num(64950.1),
        "order_quantity": num(0.002),
    },
)
add(
    "edit_order_cloid_reduce_only",
    lambda: client.edit_order(
        order_id="77",
        symbol="PERP_ETH_USDC",
        order_type="POST_ONLY",
        side="SELL",
        client_order_id="edit-9",
        order_price=1900,
        order_quantity=0.75,
        reduce_only=True,
        visible_quantity=0.25,
    ),
    {
        "order_id": 77,
        "symbol": "PERP_ETH_USDC",
        "order_type": "POST_ONLY",
        "side": "SELL",
        "client_order_id": "edit-9",
        "order_price": num(1900),
        "order_quantity": num(0.75),
        "reduce_only": True,
        "visible_quantity": num(0.25),
    },
)

# ── cancels ─────────────────────────────────────────────────────────────────

add(
    "cancel_order",
    lambda: client.cancel_order(order_id=292820969, symbol="PERP_BTC_USDC"),
    {"order_id": 292820969, "symbol": "PERP_BTC_USDC"},
)
add(
    "cancel_order_by_client_order_id",
    lambda: client.cancel_order_by_client_order_id(
        client_order_id="roshar-0001", symbol="PERP_BTC_USDC"
    ),
    {"client_order_id": "roshar-0001", "symbol": "PERP_BTC_USDC"},
)
add(
    "cancel_all_symbol",
    lambda: client.cancel_orders(symbol="PERP_ETH_USDC"),
    {"symbol": "PERP_ETH_USDC"},
)
add("cancel_all", lambda: client.cancel_orders(), {"symbol": None})
add(
    "batch_cancel",
    lambda: client.batch_cancel_orders(order_ids="101,102,103"),
    {"order_ids": [101, 102, 103]},
)
add(
    "batch_cancel_by_client_order_id",
    lambda: client.batch_cancel_orders_by_client_order_id(client_order_ids="b-1,b-2"),
    {"client_order_ids": ["b-1", "b-2"]},
)
add(
    "cancel_all_after",
    lambda: client.cancel_all_after(trigger_after=30000),
    {"trigger_after": 30000},
)

# ── reads ───────────────────────────────────────────────────────────────────

add(
    "get_orders",
    lambda: client.get_orders(
        symbol="PERP_BTC_USDC", status="INCOMPLETE", page=1, size=100
    ),
    {"symbol": "PERP_BTC_USDC", "status": "INCOMPLETE", "page": 1, "size": 100},
)
add(
    "get_orders_time_range",
    lambda: client.get_orders(
        side="SELL",
        order_type="LIMIT",
        start_t=1794700000000,
        end_t=1794780000000,
        sort_by="CREATED_TIME_ASC",
    ),
    {
        "side": "SELL",
        "order_type": "LIMIT",
        "start_t": 1794700000000,
        "end_t": 1794780000000,
        "sort_by": "CREATED_TIME_ASC",
    },
)
add("get_orders_all", lambda: client.get_orders(), {})
add("get_order", lambda: client.get_order(292820969), {"order_id": 292820969})
add(
    "get_order_by_client_order_id",
    lambda: client.get_order_by_client_order_id("roshar-0001"),
    {"client_order_id": "roshar-0001"},
)
add(
    "get_order_trades",
    lambda: client.get_all_trades_of_order(292820969),
    {"order_id": 292820969},
)
add(
    "get_trades",
    lambda: client.get_trades(symbol="PERP_ETH_USDC", start_t=1794700000000, size=50),
    {"symbol": "PERP_ETH_USDC", "start_t": 1794700000000, "size": 50},
)
add("get_trade", lambda: client.get_trade(555), {"trade_id": 555})
add("get_positions", lambda: client.get_all_positions_info(), {})
add(
    "get_position",
    lambda: client.get_one_position_info("PERP_BTC_USDC"),
    {"symbol": "PERP_BTC_USDC"},
)
add("get_holding", lambda: client.get_current_holdings(), {})
add("get_client_info", lambda: client.get_account_information(), {})
add("get_account_info", lambda: client.get_account_info(), {})
add(
    "get_funding_fee_history",
    lambda: client.get_funding_fee_history(symbol="PERP_BTC_USDC", page=2, size=20),
    {"symbol": "PERP_BTC_USDC", "page": 2, "size": 20},
)
add(
    "get_key_info",
    lambda: client.get_current_orderlykey_info(key_status="ACTIVE"),
    {"key_status": "ACTIVE"},
)
add(
    "get_orderbook",
    lambda: client.get_orderbook_snapshot("PERP_BTC_USDC", max_level=5),
    {"symbol": "PERP_BTC_USDC", "max_level": 5},
)
add(
    "get_orderbook_default_depth",
    lambda: client.get_orderbook_snapshot("PERP_ETH_USDC"),
    {"symbol": "PERP_ETH_USDC"},
)
add(
    "get_kline",
    lambda: client.get_kline("PERP_BTC_USDC", "1m", limit=10),
    {"symbol": "PERP_BTC_USDC", "type": "1m", "limit": 10},
)
add(
    "get_kline_no_limit",
    lambda: client.get_kline("PERP_ETH_USDC", "1h"),
    {"symbol": "PERP_ETH_USDC", "type": "1h"},
)

# ── websocket auth (SDK `OrderlyWebsocketClient.auth_login`) ────────────────

ws_ts = 1794780123456
ws_id = "roshar-auth-1"
fix_clock(ws_ts)
_t, ws_sign = sdk_utils.generate_signature(ORDERLY_SECRET)
ws_auth_message = json.dumps(
    {
        "id": ws_id,
        "event": "auth",
        "params": {"orderly_key": ORDERLY_KEY, "sign": ws_sign, "timestamp": int(_t)},
    }
)

# ── add orderly key (EIP-712, wallet-signed) ────────────────────────────────

add_key_out = []
for i, (chain_id, scope, ts_k, exp) in enumerate(
    [
        (42161, "read,trading", 1794780200000, 1794780200000 + 365 * 86400000),
        (8453, "trading", 1794780300000, 1794780300000 + 30 * 86400000),
    ]
):
    new_seed = secrets.token_bytes(32)
    new_pub = Ed25519PrivateKey.from_private_bytes(new_seed).public_key().public_bytes_raw()
    new_key = "ed25519:" + base58.b58encode(new_pub).decode()
    fix_clock(ts_k)
    captured.clear()
    client.add_orderly_key(
        brokerId=BROKER_ID,
        chainId=chain_id,
        orderlyKey=new_key,
        scope=scope,
        timestamp=ts_k,
        expiration=exp,
        userAddress=WALLET_ADDRESS,
    )
    assert captured["method"] == "POST"
    assert captured["url"] == BASE_URL + "/v1/orderly_key"
    params = captured["params"]
    add_key_out.append(
        {
            "broker_id": BROKER_ID,
            "chain_id": chain_id,
            "orderly_key": new_key,
            "scope": scope,
            "timestamp": ts_k,
            "expiration": exp,
            "user_address": WALLET_ADDRESS,
            "signature": params["signature"],
            "body": json.dumps(params),
        }
    )

json.dump(
    {
        "sdk": "github.com/OrderlyNetwork/orderly-evm-connector-python",
        "sdk_version": "0.2.8 (pyproject at commit)",
        "sdk_commit": sdk_commit(),
        "python": sys.version.split()[0],
        "orderly_secret": ORDERLY_SECRET,
        "orderly_key": ORDERLY_KEY,
        "wallet_private_key": WALLET_SECRET,
        "wallet_address": WALLET_ADDRESS,
        "broker_id": BROKER_ID,
        "account_id": ACCOUNT_ID,
        "requests": requests_out,
        "ws_auth": {
            "timestamp": ws_ts,
            "id": ws_id,
            "sign": ws_sign,
            "message": ws_auth_message,
        },
        "add_orderly_key": add_key_out,
    },
    sys.stdout,
    indent=2,
)
sys.stdout.write("\n")
