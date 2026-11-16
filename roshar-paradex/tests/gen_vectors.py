"""Generate tests/vectors.json from the official Paradex Python SDK (paradex-py).

Run with a virtualenv that has paradex-py installed (v0.9.0 was used):

    python tests/gen_vectors.py > tests/vectors.json

No network. The Stark private key is a fresh random throwaway key; the account address is a fixed
example value (signing only mixes it into the message hash, it need not exist). Every input the
SDK would otherwise take from the clock (signature timestamps, auth timestamp/expiry) is given
explicitly.

Each case records the inputs, the message hash from the SDK's typed-data hashing
(`typed_data_to_message_hash`), the signature from `message_signature` (starknet-crypto-py's
`sign` with an RFC 6979 nonce seeded with 32, so deterministic), `flatten_signature`, and for
orders the request body from `Order.dump_to_dict()`.
"""

import json
import secrets
import sys
from decimal import Decimal
from importlib import metadata

from starknet_py.common import int_from_bytes
from starknet_py.constants import EC_ORDER

from paradex_py.account.utils import (
    flatten_signature,
    message_signature,
    private_to_stark_key,
    typed_data_to_message_hash,
    verify_message_signature,
)
from paradex_py.common.order import Order, OrderSide, OrderType
from paradex_py.message.auth import build_auth_message
from paradex_py.message.order import build_modify_order_message, build_order_message

PRIV = secrets.randbelow(EC_ORDER - 1) + 1
PUB = private_to_stark_key(PRIV)
ACCOUNT = 0x129F3DC1B8962D8A87ABC692424C78FDA963ADE0E1CD17BF3D1C26F8D41EE7A

CHAINS = ["PRIVATE_SN_PARACLEAR_MAINNET", "PRIVATE_SN_PARACLEAR_TESTNET"]

AUTH = [
    (1763200000, 1763200300),
    (1763200000, 1763804800),
    (1, 2),
]

# market, type, side, size, price, client_id, instruction, reduce_only, stp, recv_window, ts_ms
ORDERS = [
    ("BTC-USD-PERP", "LIMIT", "BUY", "0.001", "65000.5", "c-1", "GTC", False, None, None, 1763200000123),
    ("ETH-USD-PERP", "LIMIT", "SELL", "1.5", "3200", "c-2", "POST_ONLY", False, "EXPIRE_MAKER", 5000, 1763200000124),
    ("BTC-USD-PERP", "MARKET", "BUY", "0.25", None, "c-3", "IOC", True, "EXPIRE_BOTH", None, 1763200000125),
    ("SOL-USD-PERP", "MARKET", "SELL", "12", None, "", "GTC", False, None, None, 1763200000126),
    # truncation toward zero beyond 8 decimals
    ("BTC-USD-PERP", "LIMIT", "BUY", "0.123456789123", "65432.123456789", "c-5", "GTC", False, None, None, 1763200000127),
    ("BTC-USD-PERP", "LIMIT", "SELL", "0.000000019", "0.000000019", "c-6", "IOC", True, "EXPIRE_TAKER", 10, 1763200000128),
    ("BTC-USD-PERP", "LIMIT", "BUY", "0.00000001", "1", "c-7", "GTC", False, None, None, 1763200000129),
    # more than 28 significant digits: Decimal.scaleb rounds to the context precision (28,
    # ROUND_HALF_EVEN) before int() truncates
    ("BTC-USD-PERP", "LIMIT", "BUY", "0.99999999999999999999999999999999", "123456789012345678901.123456789", "c-8", "GTC", False, None, None, 1763200000130),
    ("BTC-USD-PERP", "LIMIT", "SELL", "12345678901234567890.123456785", "9.9999999999999999999999999995", "c-9", "GTC", False, None, None, 1763200000131),
    ("kBONK-USD-PERP", "LIMIT", "BUY", "100000", "0.0123", "c-10", "RPI", False, None, None, 1763200000132),
    ("BTC-USD-PERP", "LIMIT", "BUY", "1.10", "100.00", "c-11", "GTC", False, None, None, 1763200000133),
]

# order_id, market, side, size, price, ts_ms
MODIFIES = [
    ("1763200000123010101010101", "BTC-USD-PERP", "BUY", "0.002", "64000.25", 1763200001000),
    ("1681462103821101699438490000", "ETH-USD-PERP", "SELL", "0.123456789", "3300.000000009", 1763200001001),
]


def sign(msg_hash):
    r, s = message_signature(msg_hash, PRIV)
    assert verify_message_signature(msg_hash, [r, s], PUB)
    return {"r": str(r), "s": str(s), "signature": flatten_signature([r, s])}


def order_case(chain_id, args):
    market, typ, side, size, price, client_id, instruction, reduce_only, stp, recv_window, ts = args
    o = Order(
        market=market,
        order_type=OrderType(typ),
        order_side=OrderSide(side),
        size=Decimal(size),
        limit_price=Decimal(price) if price is not None else Decimal(0),
        client_id=client_id,
        signature_timestamp=ts,
        instruction=instruction,
        reduce_only=reduce_only,
        recv_window=recv_window,
        stp=stp,
    )
    td = build_order_message(chain_id, o)
    h = typed_data_to_message_hash(td, ACCOUNT)
    sig = sign(h)
    o.signature = sig["signature"]
    return {
        "market": market,
        "type": typ,
        "side": side,
        "size": size,
        "price": price,
        "client_id": client_id,
        "instruction": instruction,
        "reduce_only": reduce_only,
        "stp": stp,
        "recv_window": recv_window,
        "signature_timestamp": ts,
        "chain_size": o.chain_size(),
        "chain_price": o.chain_price(),
        "message_hash": hex(h),
        **sig,
        "body": o.dump_to_dict(),
    }


def modify_case(chain_id, args):
    order_id, market, side, size, price, ts = args
    o = Order(
        market=market,
        order_type=OrderType.Limit,
        order_side=OrderSide(side),
        size=Decimal(size),
        limit_price=Decimal(price),
        signature_timestamp=ts,
        order_id=order_id,
    )
    td = build_modify_order_message(chain_id, o)
    h = typed_data_to_message_hash(td, ACCOUNT)
    sig = sign(h)
    o.signature = sig["signature"]
    return {
        "id": order_id,
        "market": market,
        "side": side,
        "size": size,
        "price": price,
        "signature_timestamp": ts,
        "chain_size": o.chain_size(),
        "chain_price": o.chain_price(),
        "message_hash": hex(h),
        **sig,
        "body": o.dump_to_dict(),
    }


out = {
    "generator": f"paradex-py {metadata.version('paradex-py')}, starknet-crypto-py {metadata.version('starknet-crypto-py')}",
    "private_key": hex(PRIV),
    "public_key": hex(PUB),
    "account": hex(ACCOUNT),
    "chains": {},
}

for name in CHAINS:
    chain_id = int_from_bytes(name.encode())
    c = {"chain_id": hex(chain_id), "auth": [], "orders": [], "modifies": []}
    for ts, exp in AUTH:
        h = typed_data_to_message_hash(build_auth_message(chain_id, ts, exp), ACCOUNT)
        c["auth"].append({"timestamp": ts, "expiration": exp, "message_hash": hex(h), **sign(h)})
    for args in ORDERS:
        c["orders"].append(order_case(chain_id, args))
    for args in MODIFIES:
        c["modifies"].append(modify_case(chain_id, args))
    out["chains"][name] = c

json.dump(out, sys.stdout, indent=1)
sys.stdout.write("\n")
