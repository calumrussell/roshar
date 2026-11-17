"""Generate signing vectors for roshar-edgex from the official edgeX Python SDK.

Uses `edgex-python-sdk` 2.0.1 (https://github.com/edgex-Tech/edgex-python-sdk, commit 487274d9).
No network: the HTTP session, the websocket connect, the metadata fetch and the oracle quote are
replaced with in-process fakes, and `time.time` is pinned. The trading key, API key/secret and
passphrase are random and thrown away.

    python3 -m venv /tmp/edgex-venv
    /tmp/edgex-venv/bin/pip install edgex-python-sdk==2.0.1
    /tmp/edgex-venv/bin/python tests/gen_vectors.py > tests/vectors.json
"""

import asyncio
import json
import secrets
import sys
from decimal import Decimal
from unittest import mock

import edgex_sdk.internal.async_client as ac_mod
import edgex_sdk.ws.client as ws_mod
from eth_account.messages import encode_typed_data
from eth_utils import keccak

from edgex_sdk import Client
from edgex_sdk.order.client import Client as OrderClient
from edgex_sdk.order.types import (
    CancelOrderParams,
    CreateOrderParams,
    GetActiveOrderParams,
    GetHistoryOrderPageParams,
    OrderFillTransactionParams,
    OrderSide,
    OrderType,
)
from edgex_sdk.account.client import SetMarginModeParams

BASE_URL = "https://edgex-prod-v2.edgex.exchange"
NOW = 1794780000.123  # pinned time.time()

# Trimmed from live GET /api/v2/public/meta/getMetaData (2026-11-15). 39999999 is a synthetic
# zero-fee copy of BTCUSDC to exercise a zero fee bound.
GLOBAL = {
    "chainId": "0xd0f",
    "nativeChainId": "3343",
    "contractAddress": "0xeeb3fb05cca17745cbdf193a9b16537efb68fac8",
}
COINS = [{"coinId": "1000", "coinName": "USDC", "resolution": "0xf4240", "stepSize": "0.000001"}]


def contract(cid, name, res, tick, maker="0.0004", taker="0.00045"):
    return {
        "contractId": cid,
        "contractName": name,
        "quoteCoinId": "1000",
        "resolution": res,
        "tickSize": tick,
        "defaultMakerFeeRate": maker,
        "defaultTakerFeeRate": taker,
    }


CONTRACTS = [
    contract("30000001", "BTCUSDC", "10000000000", "0.1"),
    contract("30000002", "ETHUSDC", "1000000000", "0.01"),
    contract("30000046", "1000PEPEUSDC", "100000", "0.0000001"),
    contract("39999999", "ZEROFEE", "10000000000", "0.1", maker="0", taker="0"),
]
METADATA = {"code": "SUCCESS", "data": {"global": GLOBAL, "coinList": COINS, "contractList": CONTRACTS}}
ORACLE = {"30000001": "64984.035737476061920173", "30000002": "3201.5"}


class FakeResponse:
    status = 200

    async def json(self):
        return {"code": "SUCCESS", "data": None}

    async def __aenter__(self):
        return self

    async def __aexit__(self, *a):
        return False


class FakeSession:
    closed = False

    def __init__(self):
        self.calls = []

    def request(self, method, url, json=None, params=None, headers=None):
        self.calls.append({"method": method, "url": url, "json": json, "params": params, "headers": headers})
        return FakeResponse()

    async def close(self):
        pass


SESSION = FakeSession()
TYPED = []
_real_sign = ac_mod.sign_typed_data


def recording_sign(key, typed_data, include_0x=False):
    sig = _real_sign(key, typed_data, include_0x=include_0x)
    signable = encode_typed_data(full_message=typed_data)
    digest = keccak(b"\x19" + signable.version + signable.header + signable.body)
    msg = json.loads(json.dumps(typed_data["message"], default=lambda b: "0x" + b.hex()))
    TYPED.append({"primary_type": typed_data["primaryType"], "domain": typed_data["domain"],
                  "message": msg, "digest": "0x" + digest.hex(), "signature": sig})
    return sig


L2_PRICES = []
_real_create = OrderClient.create_order


async def recording_create(self, params, metadata, l2_price):
    L2_PRICES.append(str(l2_price))
    return await _real_create(self, params, metadata, l2_price)


async def fake_ensure_session(self):
    self._session = SESSION


async def fake_metadata(self):
    return METADATA


async def fake_quote(self, contract_id):
    return {"code": "SUCCESS", "data": [{"contractId": contract_id, "oraclePrice": ORACLE[contract_id]}]}


def take_call():
    call = SESSION.calls.pop()
    assert not SESSION.calls
    return call


def take_typed():
    t = TYPED.pop()
    assert not TYPED
    return t


async def main():
    trading_key = "0x" + secrets.token_hex(32)
    creds = {
        "account_id": 700000000000000000 + secrets.randbelow(10**17),
        "api_key": secrets.token_hex(16),
        "api_passphrase": secrets.token_hex(8),
        "api_secret": secrets.token_urlsafe(32),
        "trading_private_key": trading_key,
    }
    client = Client(base_url=BASE_URL, **creds)
    signer = client.async_client.resolve_trading_signer_address()

    out = {
        "sdk": "edgex-python-sdk 2.0.1",
        "now_s": NOW,
        "base_url": BASE_URL,
        "account_id": str(creds["account_id"]),
        "api_key": creds["api_key"],
        "api_passphrase": creds["api_passphrase"],
        "api_secret": creds["api_secret"],
        "trading_private_key": trading_key,
        "signer": signer,
        "metadata": METADATA["data"],
        "oracle_prices": ORACLE,
        "calc_nonce": [],
        "orders": [],
        "requests": [],
        "margin_mode": [],
        "private_ws": None,
    }

    for cid in ["1", "1794780000000000001", "my-order-42", "x" * 64, "édge"]:
        out["calc_nonce"].append({"client_order_id": cid, "nonce": str(ac_mod.AsyncClient.calc_nonce(cid))})

    order_cases = [
        dict(contract_id="30000001", price="64979.5", size="0.001", type=OrderType.LIMIT, side=OrderSide.BUY,
             time_in_force="GOOD_TIL_CANCEL", client_order_id="1794780000000000001"),
        dict(contract_id="30000001", price="65010.0", size="0.010", type=OrderType.LIMIT, side=OrderSide.SELL,
             time_in_force="POST_ONLY", client_order_id="post-only-1", expire_time=1797372000000,
             reduce_only=True),
        dict(contract_id="30000002", price="3200.50", size="0.0100", type=OrderType.LIMIT, side=OrderSide.BUY,
             time_in_force="IMMEDIATE_OR_CANCEL", client_order_id="ioc-eth-7"),
        dict(contract_id="30000002", price="3199.99", size="12.34", type=OrderType.LIMIT, side=OrderSide.SELL,
             time_in_force="FILL_OR_KILL", client_order_id="fok-eth-8"),
        dict(contract_id="30000046", price="0.0123457", size="1000", type=OrderType.LIMIT, side=OrderSide.BUY,
             time_in_force="GOOD_TIL_CANCEL", client_order_id="pepe-1"),
        dict(contract_id="39999999", price="64979.5", size="0.5", type=OrderType.LIMIT, side=OrderSide.BUY,
             time_in_force="GOOD_TIL_CANCEL", client_order_id="zero-fee"),
        dict(contract_id="30000001", price="0", size="0.002", type=OrderType.MARKET, side=OrderSide.BUY,
             client_order_id="mkt-buy-1"),
        dict(contract_id="30000001", price="0", size="0.002", type=OrderType.MARKET, side=OrderSide.SELL,
             client_order_id="mkt-sell-1", reduce_only=True),
        dict(contract_id="30000001", price="66000.0", size="0.001", type=OrderType.STOP_LIMIT, side=OrderSide.BUY,
             client_order_id="stop-limit-1", trigger_price="65900.0", trigger_price_type="ORACLE_PRICE"),
        dict(contract_id="30000002", price="0", size="0.05", type=OrderType.TAKE_PROFIT_MARKET, side=OrderSide.SELL,
             client_order_id="tp-mkt-1", trigger_price="3500", reduce_only=True),
    ]
    for case in order_cases:
        await client.create_order(CreateOrderParams(**case))
        call = take_call()
        typed = take_typed()
        l2_price = L2_PRICES.pop()
        contract_ = next(c for c in CONTRACTS if c["contractId"] == case["contract_id"])
        fee_rate = str(max(Decimal(contract_["defaultTakerFeeRate"]), Decimal(contract_["defaultMakerFeeRate"])))
        out["orders"].append({
            "input": {k: (v.value if hasattr(v, "value") else v) for k, v in case.items()},
            "l2_price": l2_price,
            "fee_rate": fee_rate,
            "typed_data": typed,
            "request": call,
        })

    async def req(label, coro):
        await coro
        call = take_call()
        out["requests"].append({"label": label, "request": call})

    await req("cancel_by_id", client.cancel_order(CancelOrderParams(order_id="816966711830904982")))
    await req("cancel_by_client_id", client.order.cancel_order_by_client_order_id(["a-1", "b-2"]))
    await req("cancel_all_contract", client.cancel_order(CancelOrderParams(contract_id="30000001")))
    await req("cancel_all_filters", client.async_client.make_authenticated_request(
        method="POST", path="/api/v2/private/order/cancelAllOrder",
        data={"accountId": str(creds["account_id"]), "filterCoinIdList": [], "filterContractIdList": ["30000001", "30000002"],
              "filterOrderTypeList": ["LIMIT"], "filterOrderStatusList": ["OPEN", "UNTRIGGERED"],
              "filterIsPositionTpsl": [False]}))
    await req("cancel_all_empty", client.async_client.make_authenticated_request(
        method="POST", path="/api/v2/private/order/cancelAllOrder",
        data={"accountId": str(creds["account_id"])}))
    await req("active_orders", client.get_active_orders(GetActiveOrderParams(
        size="50", filter_contract_id_list=["30000001", "30000002"], filter_status_list=["OPEN"],
        filter_is_position_tpsl=False)))
    await req("active_orders_plain", client.get_active_orders(GetActiveOrderParams()))
    await req("history_orders", client.order.get_history_order_page(GetHistoryOrderPageParams(
        size=20, offset_data="0880A08A97B532", filter_type_list=["LIMIT", "MARKET"])))
    await req("fills", client.get_order_fill_transactions(OrderFillTransactionParams(
        size="100", filter_order_id_list=["816966711830904982"], filter_start_created_time_inclusive=1794700000000)))
    await req("orders_by_id", client.order.get_orders_by_id(["1", "2"]))
    await req("orders_by_client_id", client.order.get_orders_by_client_order_id(["a-1"]))
    await req("max_order_size", client.get_max_order_size("30000001", Decimal("65000")))
    await req("account_asset", client.get_account_asset())
    await req("account_by_id", client.account.get_account_by_id())
    await req("positions_by_contract", client.account.get_position_by_contract_id(["30000001", "30000002"]))
    await req("collateral_by_coin", client.account.get_collateral_by_coin_id(["1000"]))
    await req("update_leverage", client.account.update_leverage_setting("30000001", "20"))

    for contract_id, mode, cid in [("30000001", "1", "mm-1"), ("30000002", "0", "mm-2")]:
        await client.set_margin_mode(SetMarginModeParams(contract_id=contract_id, margin_mode=mode, client_order_id=cid))
        out["margin_mode"].append({
            "input": {"contract_id": contract_id, "margin_mode": mode, "client_order_id": cid},
            "typed_data": take_typed(),
            "request": take_call(),
        })

    captured = {}

    def fake_connect(url, header=None, subprotocols=None):
        captured.update({"url": url, "headers": header, "subprotocols": subprotocols})
        raise RuntimeError("no network")

    ws = ws_mod.Client(url="wss://edgex-quote-prod-v2.edgex.exchange/api/v1/private/ws", is_private=True,
                       account_id=creds["account_id"], api_key=creds["api_key"],
                       api_passphrase=creds["api_passphrase"], api_secret=creds["api_secret"])
    with mock.patch.object(ws_mod.websocket, "create_connection", fake_connect):
        try:
            ws.connect()
        except ValueError:
            pass
    out["private_ws"] = captured

    json.dump(out, sys.stdout, indent=1, ensure_ascii=False)
    sys.stdout.write("\n")


with mock.patch("time.time", return_value=NOW), \
        mock.patch.object(ac_mod.AsyncClient, "_ensure_session", fake_ensure_session), \
        mock.patch.object(ac_mod, "sign_typed_data", recording_sign), \
        mock.patch.object(OrderClient, "create_order", recording_create), \
        mock.patch.object(Client, "get_metadata", fake_metadata), \
        mock.patch.object(Client, "get_24_hour_quote", fake_quote):
    asyncio.run(main())
