"""Generates tests/vectors.json with the official Pacifica Python SDK's signer.

    git clone https://github.com/pacifica-fi/python-sdk /tmp/pacifica-python-sdk
    pip install solders base58
    python tests/gen_vectors.py /tmp/pacifica-python-sdk > tests/vectors.json

The keys are throwaway keys derived from fixed seeds and the script makes no network calls. Never
regenerate the vectors to make a change pass.
"""

import hashlib
import json
import subprocess
import sys

sdk_path = sys.argv[1]
sys.path.insert(0, sdk_path)

from solders.keypair import Keypair  # noqa: E402
from common.utils import sign_message  # noqa: E402

import base58  # noqa: E402


def keypair(label):
    return Keypair.from_seed(hashlib.sha256(label.encode()).digest())


signer = keypair("roshar-pacifica vector signer")
other_account = keypair("roshar-pacifica vector main account")

# (name, type, payload in SDK insertion order, account, timestamp, expiry_window, builder inputs)
CASES = [
    (
        "create_order_cloid_main_key",
        "create_order",
        {
            "symbol": "BTC",
            "price": "100000",
            "reduce_only": False,
            "amount": "0.1",
            "side": "bid",
            "tif": "GTC",
            "client_order_id": "12345678-1234-1234-1234-123456789abc",
        },
        str(signer.pubkey()),
        1748970123456,
        5000,
        {
            "kind": "limit",
            "symbol": "BTC",
            "price": 100000.0,
            "amount": 0.1,
            "is_buy": True,
            "tif": "GTC",
            "reduce_only": False,
            "client_order_id": "12345678-1234-1234-1234-123456789abc",
        },
    ),
    (
        "create_order_agent_key",
        "create_order",
        {
            "symbol": "kPEPE",
            "price": "0.003849",
            "reduce_only": True,
            "amount": "26000",
            "side": "ask",
            "tif": "ALO",
        },
        str(other_account.pubkey()),
        1794779298111,
        30000,
        {
            "kind": "limit",
            "symbol": "kPEPE",
            "price": 0.003849,
            "amount": 26000.0,
            "is_buy": False,
            "tif": "ALO",
            "reduce_only": True,
            "client_order_id": None,
        },
    ),
    (
        "cancel_order",
        "cancel_order",
        {"symbol": "ETH", "order_id": 14589281256},
        str(signer.pubkey()),
        1794779443791,
        5000,
        {"kind": "cancel", "symbol": "ETH", "order_id": 14589281256},
    ),
    (
        "edit_order_agent_key",
        "edit_order",
        {"symbol": "SOL", "price": "100.67", "amount": "12.5", "order_id": 315979358},
        str(other_account.pubkey()),
        1794779500000,
        5000,
        {
            "kind": "edit",
            "symbol": "SOL",
            "price": 100.67,
            "amount": 12.5,
            "order_id": 315979358,
        },
    ),
    # Signing only: nested objects and lists are sorted at every level, null is kept, and
    # non-ASCII characters (and DEL) are escaped as json.dumps does by default.
    (
        "sorting_and_escaping",
        "create_order",
        {
            "symbol": "BTC",
            "take_profit": {
                "stop_price": "55000",
                "limit_price": "54950",
                "trigger_price_type": "mark_price",
            },
            "order_expiry": None,
            "batch": [{"z": 1, "a": [{"y": "q", "b": False}]}],
            "note": "caf\u00e9 \u2713 \U0001f600 \u007f \"quoted\" back\\slash",
        },
        str(signer.pubkey()),
        1700000000000,
        5000,
        None,
    ),
]

vectors = []
for name, typ, payload, account, timestamp, expiry_window, inputs in CASES:
    header = {"timestamp": timestamp, "expiry_window": expiry_window, "type": typ}
    message, signature = sign_message(header, payload, signer)
    # Request as the SDK builds it (rest/api_agent_keys.py): agent_wallet only for an agent key.
    request = {
        "account": account,
        "signature": signature,
        "timestamp": timestamp,
        "expiry_window": expiry_window,
    }
    if account != str(signer.pubkey()):
        request["agent_wallet"] = str(signer.pubkey())
    request.update(payload)
    vectors.append(
        {
            "name": name,
            "type": typ,
            "timestamp": timestamp,
            "expiry_window": expiry_window,
            "account": account,
            "payload": payload,
            "inputs": inputs,
            "message": message,
            "signature": signature,
            "request": request,
        }
    )

sdk_commit = subprocess.run(
    ["git", "-C", sdk_path, "rev-parse", "HEAD"], capture_output=True, text=True
).stdout.strip()

print(
    json.dumps(
        {
            "sdk": "github.com/pacifica-fi/python-sdk",
            "sdk_commit": sdk_commit,
            "private_key_b58": base58.b58encode(bytes(signer)).decode("ascii"),
            "public_key": str(signer.pubkey()),
            "vectors": vectors,
        },
        indent=2,
        ensure_ascii=False,
    )
)
