"""Generates tests/vectors.json with the official ApeX Omni Python SDK (`apexomni` 3.4.2, which ships
the zkLink signer as a native library).

    uv venv /tmp/apex-venv -p 3.12
    VIRTUAL_ENV=/tmp/apex-venv uv pip install apexomni==3.4.2
    /tmp/apex-venv/bin/python tests/gen_vectors.py > tests/vectors.json

Keys, seeds and API credentials are throwaway values derived from fixed labels. The script makes no
network calls: `create_order_v3` runs against an in-memory config/account and a captured `_post`.
Never regenerate the vectors to make a change pass.
"""

import base64
import hashlib
import json
import warnings

warnings.filterwarnings("ignore")

from apexomni import zklink_sdk as sdk  # noqa: E402
from apexomni import http_private_sign  # noqa: E402
from apexomni import _websocket_stream  # noqa: E402
from apexomni.constants import APEX_OMNI_HTTP_MAIN, NETWORKID_OMNI_MAIN_ARB  # noqa: E402
from apexomni.http_private_sign import HttpPrivateSign  # noqa: E402
from importlib.metadata import version  # noqa: E402


def label_bytes(label, n):
    out = b""
    counter = 0
    while len(out) < n:
        out += hashlib.sha256(f"{label}/{counter}".encode()).digest()
        counter += 1
    return out[:n]


# Seeds are the 65-byte personal_sign signature the SDK derives them from; any 65 bytes work.
SEEDS = [label_bytes(f"roshar-apex seed {i}", 65).hex() for i in range(3)]
CREDENTIALS = {
    "key": "3f1c8a52-7e0b-4d6f-9a21-5c0e8b7d4a19",
    "secret": base64.urlsafe_b64encode(label_bytes("roshar-apex secret", 30)).decode(),
    "passphrase": base64.urlsafe_b64encode(label_bytes("roshar-apex passphrase", 15)).decode(),
}

# ---------------------------------------------------------------- keys

keys = []
for seeds in SEEDS:
    signer = sdk.ZkLinkSigner.new_from_seed(bytes.fromhex(seeds))
    l2_key = signer.public_key()
    keys.append({"seeds": seeds, "l2_key": l2_key, "pub_key_hash": sdk.get_public_key_hash(l2_key)})

# ---------------------------------------------------------------- raw musig

contract_bytes_example = bytes.fromhex("fe0000000100000200000304010001e84800000000000000000000000df8475800020500")
MUSIG_MESSAGES = [b"", b"abc", contract_bytes_example, bytes(range(92))]
musig = []
for i, seeds in enumerate(SEEDS):
    signer = sdk.ZkLinkSigner.new_from_seed(bytes.fromhex(seeds))
    for msg in MUSIG_MESSAGES:
        musig.append({"seeds": seeds, "message": msg.hex(), "signature": signer.sign_musig(msg).signature})

# ---------------------------------------------------------------- orders

recorded = {}


class RecordingBuilder(sdk.ContractBuilder):
    def __init__(self, *args):
        super().__init__(*args)
        recorded["builder"] = {
            "account_id": self.account_id,
            "sub_account_id": self.sub_account_id,
            "slot_id": self.slot_id,
            "nonce": self.nonce,
            "pair_id": self.pair_id,
            "size": self.size,
            "price": self.price,
            "direction": self.direction,
            "taker_fee_rate": self.taker_fee_rate,
            "maker_fee_rate": self.maker_fee_rate,
            "has_subsidy": self.has_subsidy,
        }


class RecordingContract(sdk.Contract):
    def get_bytes(self):
        out = super().get_bytes()
        recorded["contract_bytes"] = bytes(out).hex()
        return out


http_private_sign.sdk.ContractBuilder = RecordingBuilder
http_private_sign.sdk.Contract = RecordingContract


def contract(symbol, pair_id, tick):
    return {"symbol": symbol, "symbolDisplayName": symbol.replace("-", ""), "l2PairId": pair_id,
            "tickSize": tick, "settleAssetId": "USDT"}


def config(decimals):
    return {
        "contractConfig": {
            "perpetualContract": [contract("BTC-USDT", "50001", "0.1"), contract("DOGE-USDT", "50017", "0.00001")],
            "prelaunchContract": [],
            "predictionContract": [contract("Knicks_Win_Against_Celtics_Dec2-USDT", "50199", "0.001")],
            "stockContract": [],
            "assets": [{"token": "USDT", "decimals": decimals}],
        }
    }


ORDER_CASES = [
    dict(name="limit_buy_gtc", account_id="584232029671915612", taker="0.0005", maker="0.0002", decimals=18,
         args=dict(symbol="BTC-USDT", side="BUY", type="LIMIT", size="0.01", price="65000.5",
                   timeInForce="GOOD_TIL_CANCEL", clientId="123456789012345", timestampSeconds=1794780000)),
    dict(name="post_only_reduce_only_sell", account_id="584232029671915612", taker="0.00025", maker="0.00000000",
         decimals=18,
         args=dict(symbol="BTC-USDT", side="SELL", type="LIMIT", size="0.123", price="64999.9",
                   timeInForce="POST_ONLY", reduceOnly=True, clientId="apexomni-584232029671915612-1794780000000-a1b2c3",
                   timestampSeconds=1794780123)),
    dict(name="market_ioc_buy_float_slot_rounding", account_id="713292165948637192", taker="0.0005", maker="0.0002",
         decimals=18,
         args=dict(symbol="BTC-USDT", side="BUY", type="MARKET", size="1.5", price="71500.0",
                   timeInForce="IMMEDIATE_OR_CANCEL", clientId="1970091", timestampSeconds=1794780456)),
    dict(name="stop_market_fok_sell", account_id="1", taker="0.00035", maker="0.0001", decimals=18,
         args=dict(symbol="BTC-USDT", side="SELL", type="STOP_MARKET", size="0.0100", price="58500.0",
                   timeInForce="FILL_OR_KILL", reduceOnly=True, triggerPrice="60000", triggerPriceType="INDEX",
                   clientId="9007199254740993", timestampSeconds=1700000000)),
    dict(name="small_price_large_size", account_id="4294967295", taker="0.0006", maker="0.0002", decimals=18,
         args=dict(symbol="DOGE-USDT", side="BUY", type="LIMIT", size="123456", price="0.12345",
                   timeInForce="GOOD_TIL_CANCEL", clientId="doge-1", timestampSeconds=1794780789)),
    dict(name="prediction_contract_six_decimals", account_id="854931562279469100", taker="0.001", maker="0.0005",
         decimals=6,
         args=dict(symbol="Knicks_Win_Against_Celtics_Dec2-USDT", side="SELL", type="LIMIT", size="2000",
                   price="0.537", timeInForce="GOOD_TIL_CANCEL", clientId="42", timestampSeconds=1764000000)),
]

# The SDK sends these two as the string "None" when no TP/SL is attached; the crate omits them.
SDK_NONE_STRINGS = ("slLimitFee", "tpLimitFee")

orders = []
for i, case in enumerate(ORDER_CASES):
    seeds = SEEDS[i % len(SEEDS)]
    client = HttpPrivateSign(APEX_OMNI_HTTP_MAIN, network_id=NETWORKID_OMNI_MAIN_ARB, zk_seeds=seeds,
                             zk_l2Key=keys[i % len(SEEDS)]["l2_key"], api_key_credentials=CREDENTIALS)
    client.configV3 = config(case["decimals"])
    client.accountV3 = {"id": case["account_id"],
                        "contractAccount": {"takerFeeRate": case["taker"], "makerFeeRate": case["maker"]}}
    captured = {}

    def fake_post(endpoint, data, headers=None, account_type=None, captured=captured):
        captured["endpoint"] = endpoint
        captured["data"] = data
        return {}

    client._post = fake_post
    recorded.clear()
    client.create_order_v3(**case["args"])
    data = captured["data"]
    assert all(data[k] == "None" for k in SDK_NONE_STRINGS)
    form = {k: v for k, v in data.items() if v is not None and k not in SDK_NONE_STRINGS}
    timestamp_ms = 1794780000000 + i
    orders.append({
        "name": case["name"],
        "seeds": seeds,
        "account_id": case["account_id"],
        "taker_fee_rate": case["taker"],
        "maker_fee_rate": case["maker"],
        "l2_pair_id": [c for c in client.configV3["contractConfig"]["perpetualContract"]
                       + client.configV3["contractConfig"]["predictionContract"]
                       if c["symbol"] == case["args"]["symbol"]][0]["l2PairId"],
        "settle_decimals": case["decimals"],
        "inputs": {k: v for k, v in case["args"].items() if k != "timestampSeconds"},
        "builder": recorded["builder"],
        "contract_bytes": recorded["contract_bytes"],
        "signature": data["signature"],
        "limit_fee": data["limitFee"],
        "expiration": data["expiration"],
        "endpoint": captured["endpoint"],
        "form": {k: str(v) for k, v in form.items()},
        "hmac": {
            "timestamp": timestamp_ms,
            "signature": client.sign(request_path=captured["endpoint"], method="POST",
                                     iso_timestamp=str(timestamp_ms), data=form, credentials=CREDENTIALS),
        },
    })

# ---------------------------------------------------------------- request HMAC

hmac_client = HttpPrivateSign(APEX_OMNI_HTTP_MAIN, network_id=NETWORKID_OMNI_MAIN_ARB,
                              api_key_credentials=CREDENTIALS)
HMAC_CASES = [
    ("GET", "/api/v3/account", {}),
    ("GET", "/api/v3/history-orders?symbol=BTC-USDT&limit=10&page=0", {}),
    ("POST", "/api/v3/delete-order", {"id": "588326929847812444"}),
    ("POST", "/api/v3/delete-client-order-id", {"id": "123456789012345"}),
    ("POST", "/api/v3/delete-open-orders", {"symbol": "BTC-USDT,ETH-USDT"}),
    ("POST", "/api/v3/set-initial-margin-rate", {"symbol": "BTC-USDT", "initialMarginRate": "0.05"}),
]
hmac_vectors = []
for i, (method, path, data) in enumerate(HMAC_CASES):
    ts = 1794780100000 + i
    hmac_vectors.append({
        "method": method, "path": path, "data": data, "timestamp": ts,
        "signature": hmac_client.sign(request_path=path, method=method, iso_timestamp=str(ts), data=data,
                                      credentials=CREDENTIALS),
    })

# ---------------------------------------------------------------- websocket login


class WsStub:
    api_key_credentials = CREDENTIALS


ws_ts = 1794780200000
ws_login = {
    "timestamp": ws_ts,
    "request_path": _websocket_stream.PRIVATE_REQUEST_PATH,
    "signature": _websocket_stream._ApexWebSocketManager.sign(
        WsStub(), request_path=_websocket_stream.PRIVATE_REQUEST_PATH, method="GET", iso_timestamp=str(ws_ts)),
}

print(json.dumps({
    "sdk": "apexomni (PyPI) / github.com/ApeX-Protocol/apexpro-openapi",
    "sdk_version": version("apexomni"),
    "credentials": CREDENTIALS,
    "keys": keys,
    "musig": musig,
    "orders": orders,
    "hmac": hmac_vectors,
    "ws_login": ws_login,
}, indent=2))
