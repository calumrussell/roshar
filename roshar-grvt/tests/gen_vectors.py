"""Generate tests/vectors.json with GRVT's official signing code. No network.

Order signing uses `pysdk.grvt_raw_signing.sign_order` from grvt-pysdk
(github.com/gravity-technologies/grvt-pysdk, commit bb77839, the first with
`OrderWithBuilderFee`; PyPI 0.2.1 predates it). Wallet-login signing uses
`build_eip712_payload` + `sign_eip712` from GRVT's builder-examples
(github.com/gravity-technologies/builder-examples, commit 6ad03de, python/).

    python3 -m venv /tmp/grvt-venv
    /tmp/grvt-venv/bin/pip install git+https://github.com/gravity-technologies/grvt-pysdk@bb77839
    git clone https://github.com/gravity-technologies/builder-examples /tmp/builder-examples
    GRVT_BUILDER_EXAMPLES=/tmp/builder-examples/python \
        /tmp/grvt-venv/bin/python tests/gen_vectors.py > tests/vectors.json

A fresh throwaway key is drawn on every run and recorded in the output.
"""

import dataclasses
import json
import os
import secrets
import sys
from types import SimpleNamespace

from eth_account import Account
from eth_account.messages import encode_typed_data
from eth_utils import keccak

from pysdk import grvt_raw_signing as rs
from pysdk.grvt_raw_base import GrvtApiConfig
from pysdk.grvt_raw_env import GrvtEnv
from pysdk.grvt_raw_types import (
    ApiCreateOrderRequest,
    Order,
    OrderLeg,
    OrderMetadata,
    Signature,
    TimeInForce,
)

sys.path.insert(0, os.environ["GRVT_BUILDER_EXAMPLES"])
import wallet_login as wl  # noqa: E402
from grvt_common import sign_eip712  # noqa: E402

# `sign_order` reads only `instrument_hash` and `base_decimals`. Values from the live
# `full/v1/instruments` capture in tests/fixtures/instruments_all.json.
INSTRUMENTS = {
    "BTC_USDT_Perp": SimpleNamespace(instrument_hash="0x030501", base_decimals=9),
    "ETH_USDT_Perp": SimpleNamespace(instrument_hash="0x030401", base_decimals=9),
    "XAG_USDC_SFP": SimpleNamespace(instrument_hash="0x025609", base_decimals=6),
    "USDC_USDT_SpotSwap": SimpleNamespace(instrument_hash="0x030208", base_decimals=6),
}

ENVS = {325: GrvtEnv.PROD, 326: GrvtEnv.TESTNET}


def drop_nulls(v):
    if isinstance(v, dict):
        return {k: drop_nulls(x) for k, x in v.items() if x is not None}
    if isinstance(v, list):
        return [drop_nulls(x) for x in v]
    return v


def order_case(name, key, chain_id, sub_account_id, tif, legs, nonce, expiration,
               client_order_id, is_market=False, post_only=False, reduce_only=False,
               builder=None, builder_fee=None):
    order = Order(
        sub_account_id=sub_account_id,
        time_in_force=tif,
        legs=[OrderLeg(instrument=i, size=s, is_buying_asset=b, limit_price=p) for (i, s, p, b) in legs],
        signature=Signature(signer="", r="", s="", v=0, expiration=str(expiration), nonce=nonce),
        metadata=OrderMetadata(client_order_id=client_order_id),
        is_market=is_market,
        post_only=post_only,
        reduce_only=reduce_only,
        builder=builder,
        builder_fee=builder_fee,
    )
    config = GrvtApiConfig(env=ENVS[chain_id], trading_account_id=None, private_key=key,
                           api_key=None, logger=None)
    account = Account.from_key(key)

    # Intermediates, built exactly as `sign_order` builds them.
    domain = rs.get_EIP712_domain_data(config.env, rs.CHAIN_IDS[config.env])
    if rs._has_builder(order):
        types = rs.EIP712_ORDER_WITH_BUILDER_FEE_MESSAGE_TYPE
        message = rs.build_EIP712_order_with_builder_fee_message_data(order, INSTRUMENTS)
    else:
        types = rs.EIP712_ORDER_MESSAGE_TYPE
        message = rs.build_EIP712_order_message_data(order, INSTRUMENTS)
    signable = encode_typed_data(domain, types, message)

    signed = rs.sign_order(order, config, account, INSTRUMENTS)
    # Request body as the SDK's `create_order_v1` serialises it (DataclassJSONEncoder ==
    # dataclasses.asdict + Enum.value), with unset (null) fields dropped.
    body = json.loads(json.dumps(ApiCreateOrderRequest(order=signed), cls=_Enc))
    return {
        "name": name,
        "chain_id": chain_id,
        "instruments": {k: vars(v) for k, v in INSTRUMENTS.items()},
        "domain_separator": "0x" + signable.header.hex(),
        "struct_hash": "0x" + signable.body.hex(),
        "digest": "0x" + keccak(b"\x19\x01" + signable.header + signable.body).hex(),
        "message": {k: (str(v) if isinstance(v, int) and not isinstance(v, bool) else v)
                    for k, v in message.items() if k != "legs"},
        "request": drop_nulls(body),
    }


class _Enc(json.JSONEncoder):
    def default(self, o):
        if dataclasses.is_dataclass(o):
            return dataclasses.asdict(o)
        if hasattr(o, "value"):
            return o.value
        return super().default(o)


def login_case(name, key, chain_id, nonce, expiration):
    addr = Account.from_key(key).address
    typed = wl.build_eip712_payload(addr, nonce, expiration, chain_id)
    signable = encode_typed_data(full_message=typed)
    v, r, s = sign_eip712(key, typed)
    # Body as `wallet_login.wallet_login` posts it.
    body = {
        "address": addr,
        "signature": {
            "signer": addr,
            "v": v,
            "r": r,
            "s": s,
            "nonce": nonce,
            "expiration": str(expiration),
            "chain_id": str(chain_id),
        },
    }
    return {
        "name": name,
        "chain_id": chain_id,
        "nonce": nonce,
        "expiration": str(expiration),
        "domain_separator": "0x" + signable.header.hex(),
        "struct_hash": "0x" + signable.body.hex(),
        "digest": "0x" + keccak(b"\x19\x01" + signable.header + signable.body).hex(),
        "request": body,
    }


def main():
    key = "0x" + secrets.token_hex(32)
    address = Account.from_key(key).address
    G = TimeInForce.GOOD_TILL_TIME
    I = TimeInForce.IMMEDIATE_OR_CANCEL
    F = TimeInForce.FILL_OR_KILL
    A = TimeInForce.ALL_OR_NONE
    orders = [
        order_case("gtt_post_only_buy", key, 325, "4110889093865147", G,
                   [("BTC_USDT_Perp", "0.001", "65038.1", True)], 305419896,
                   1794780000000000000, "9223372036854775809", post_only=True),
        order_case("ioc_reduce_only_sell", key, 325, "4110889093865147", I,
                   [("ETH_USDT_Perp", "1.25", "3120.55", False)], 1, 1794780000123456789,
                   "18446744073709551615", reduce_only=True),
        order_case("fok_market_buy", key, 325, "77", F,
                   [("BTC_USDT_Perp", "0.5", "0", True)], 4294967295, 1794780000000000001,
                   "12345", is_market=True),
        order_case("aon_testnet", key, 326, "18446744073709551615", A,
                   [("ETH_USDT_Perp", "10", "2999.999999999", True)], 0, 1, "1"),
        order_case("six_decimal_base", key, 325, "4110889093865147", G,
                   [("XAG_USDC_SFP", "50.000001", "31.123", False)], 42, 1794780000000000000,
                   "9300000000000000000"),
        order_case("spot_swap", key, 325, "4110889093865147", I,
                   [("USDC_USDT_SpotSwap", "1000", "0.99991", True)], 7, 1794780000000000000,
                   "9300000000000000001"),
        order_case("two_legs", key, 325, "4110889093865147", G,
                   [("BTC_USDT_Perp", "0.002", "64000", True),
                    ("ETH_USDT_Perp", "0.03", "3100.5", False)], 99, 1794780000000000000,
                   "9300000000000000002"),
        order_case("builder_fee", key, 325, "4110889093865147", G,
                   [("BTC_USDT_Perp", "0.1", "65000.5", True)], 123456, 1794780000000000000,
                   "9300000000000000003", builder="0xdc93ed3bc9932915a5bc7f6313b946091afc7e86",
                   builder_fee="0.001"),
        order_case("builder_fee_testnet_ioc", key, 326, "4110889093865147", I,
                   [("ETH_USDT_Perp", "0.25", "3001", False)], 9, 1794780000000000000,
                   "9300000000000000004", reduce_only=True,
                   builder="0x00000000000000000000000000000000000000FF", builder_fee="0.0125"),
    ]
    logins = [
        login_case("login_prod", key, 325, 305419896, 1794780300000000000),
        login_case("login_testnet", key, 326, 4294967295, 1794780000000000001),
    ]
    json.dump({"private_key": key, "address": address, "orders": orders, "wallet_logins": logins},
              sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
