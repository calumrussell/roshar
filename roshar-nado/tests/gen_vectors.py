"""Generate signing vectors for roshar-nado with the official Python SDK.

    uv venv /tmp/nado-venv && VIRTUAL_ENV=/tmp/nado-venv uv pip install nado-protocol==2.0.0
    /tmp/nado-venv/bin/python tests/gen_vectors.py > tests/vectors.json

A fresh random key is used on every run and nothing touches the network: the engine client's
`_execute` is replaced with a function that records the request body the SDK would POST to
`/execute`. Signatures are RFC 6979 (deterministic), so the Rust signer must reproduce them exactly.
"""

import json
import secrets
import sys

import nado_protocol
from eth_account import Account

from nado_protocol.contracts.eip712.sign import (
    build_eip712_typed_data,
    get_eip712_typed_data_digest,
    sign_eip712_typed_data,
)
from nado_protocol.contracts.types import NadoExecuteType, NadoTxType
from nado_protocol.engine_client.execute import EngineExecuteClient
from nado_protocol.engine_client.types import EngineClientOpts
from nado_protocol.engine_client.types.execute import (
    CancelAndPlaceParams,
    CancelOrdersParams,
    CancelProductOrdersParams,
    LinkSignerParams,
    OrderParams,
    PlaceOrderParams,
    PlaceOrdersParams,
)
from nado_protocol.engine_client.types.stream import StreamAuthenticationParams
from nado_protocol.utils.bytes32 import subaccount_to_bytes32, subaccount_to_hex
from nado_protocol.utils.expiration import OrderType
from nado_protocol.utils.order import OrderAppendixTriggerType, build_appendix
from nado_protocol.ws import messages

# Live `contracts` query results (2026-11-15).
NETWORKS = {
    "mainnet": (57073, "0x05ec92d78ed421f3d3ada77ffde167106565974e"),
    "testnet": (763373, "0x698d87105274292b5673367dec81874ce3633ac2"),
}

I128_MAX = (1 << 127) - 1
I128_MIN = -(1 << 127)
U64_MAX = (1 << 64) - 1
U128_MAX = (1 << 128) - 1


def client_for(key_hex, network):
    chain_id, endpoint = NETWORKS[network]
    client = EngineExecuteClient(
        EngineClientOpts(
            url="http://offline.invalid/v1",
            signer=key_hex,
            chain_id=chain_id,
            endpoint_addr=endpoint,
        )
    )
    captured = []

    def fake_execute(req):
        captured.append(json.loads(json.dumps(req.dict())))
        return None

    client._execute = fake_execute
    return client, captured


def order(sender, price_x18, amount, expiration, nonce, appendix):
    return OrderParams(
        sender=sender,
        priceX18=price_x18,
        amount=amount,
        expiration=expiration,
        nonce=nonce,
        appendix=appendix,
    )


def main():
    key = "0x" + secrets.token_hex(32)
    acct = Account.from_key(key)
    other = Account.from_key("0x" + secrets.token_hex(32))
    default_sub = subaccount_to_hex(acct.address, "default")
    bot_sub = subaccount_to_hex(acct.address, "bot1")
    out = {
        "sdk": f"nado-protocol {getattr(nado_protocol, '__version__', '2.0.0')}",
        "private_key": key,
        "address": acct.address.lower(),
        "networks": {k: {"chain_id": c, "endpoint_addr": e} for k, (c, e) in NETWORKS.items()},
        "subaccounts": [
            {"address": acct.address.lower(), "name": "default", "bytes32": default_sub},
            {"address": acct.address.lower(), "name": "bot1", "bytes32": bot_sub},
            {"address": acct.address.lower(), "name": "", "bytes32": subaccount_to_hex(acct.address, "")},
            {"address": acct.address.lower(), "name": "abcdefghijkl", "bytes32": subaccount_to_hex(acct.address, "abcdefghijkl")},
        ],
        "appendix": [],
        "place_order": [],
        "place_orders": [],
        "cancel_orders": [],
        "cancel_product_orders": [],
        "cancel_and_place": [],
        "link_signer": [],
        "stream_auth": [],
    }

    appendix_cases = [
        dict(order_type=OrderType.DEFAULT),
        dict(order_type=OrderType.IOC),
        dict(order_type=OrderType.FOK, reduce_only=True),
        dict(order_type=OrderType.POST_ONLY),
        dict(order_type=OrderType.POST_ONLY, reduce_only=True),
        dict(order_type=OrderType.DEFAULT, isolated=True, isolated_margin=25_000_000),
        dict(order_type=OrderType.IOC, isolated=True),
        dict(order_type=OrderType.DEFAULT, builder_id=7, builder_fee_rate=15),
        dict(order_type=OrderType.FOK, isolated=True, isolated_margin=U64_MAX, reduce_only=True, builder_id=65535, builder_fee_rate=1023),
        dict(order_type=OrderType.DEFAULT, trigger_type=OrderAppendixTriggerType.PRICE),
    ]
    for case in appendix_cases:
        value = build_appendix(**case)
        out["appendix"].append(
            {
                "order_type": int(case["order_type"]),
                "isolated": case.get("isolated", False),
                "reduce_only": case.get("reduce_only", False),
                "trigger_type": int(case["trigger_type"]) if case.get("trigger_type") else 0,
                "isolated_margin": case.get("isolated_margin"),
                "builder_id": case.get("builder_id"),
                "builder_fee_rate": case.get("builder_fee_rate"),
                "appendix": str(value),
            }
        )

    # place_order
    place_cases = [
        ("mainnet", dict(id=101, product_id=2, order=order(default_sub, 65_000 * 10**18, 15 * 10**16, 4294967295, (1794780000000 << 20) + 17, build_appendix(OrderType.POST_ONLY)))),
        ("mainnet", dict(product_id=4, spot_leverage=False, order=order(default_sub, 1_800 * 10**18, -3 * 10**18, 1794780100, (1794780000050 << 20) + 999, build_appendix(OrderType.IOC, reduce_only=True)))),
        ("mainnet", dict(id=7, product_id=2, order=order(bot_sub, 64_000 * 10**18, 10**15, 1794790000, (1794780000100 << 20), build_appendix(OrderType.FOK, isolated=True, isolated_margin=50_000_000, builder_id=3, builder_fee_rate=10)))),
        ("mainnet", dict(product_id=1, spot_leverage=True, order=order(default_sub, 65_123_456_789_000_000_000_000, 5 * 10**13, 4294967295, (1794780000200 << 20) + 1048575, build_appendix(OrderType.DEFAULT)))),
        ("mainnet", dict(product_id=4294967295, order=order(default_sub, I128_MAX, I128_MIN, U64_MAX, U64_MAX, U128_MAX))),
        # Product 0 (USDT0) has no book; the SDK would also fall back to the endpoint address for it.
        ("mainnet", dict(product_id=3, order=order(default_sub, -1, -1, 0, 0, 0))),
        ("testnet", dict(id=1, product_id=2, order=order(default_sub, 65_000 * 10**18, 15 * 10**16, 4294967295, (1794780000000 << 20) + 17, build_appendix(OrderType.POST_ONLY)))),
    ]
    for network, params in place_cases:
        client, captured = client_for(key, network)
        p = PlaceOrderParams(**params)
        digest = client.get_order_digest(p.order, p.product_id)
        client.place_order(p)
        o = params["order"]
        out["place_order"].append(
            {
                "network": network,
                "product_id": params["product_id"],
                "id": params.get("id"),
                "spot_leverage": params.get("spot_leverage"),
                "sender": subaccount_to_hex(o.sender),
                "price_x18": str(o.priceX18),
                "amount": str(o.amount),
                "expiration": str(o.expiration),
                "nonce": str(o.nonce),
                "appendix": str(o.appendix),
                "digest": digest,
                "request": captured[0],
            }
        )

    # place_orders (batch)
    client, captured = client_for(key, "mainnet")
    batch = [
        dict(id=11, product_id=2, order=order(default_sub, 64_900 * 10**18, 2 * 10**16, 4294967295, (1794780001000 << 20) + 1, build_appendix(OrderType.POST_ONLY))),
        dict(product_id=4, order=order(default_sub, 1_900 * 10**18, -10**18, 4294967295, (1794780001000 << 20) + 2, build_appendix(OrderType.POST_ONLY, reduce_only=True))),
    ]
    digests = []
    for b in batch:
        p = PlaceOrderParams(**b)
        digests.append(client.get_order_digest(p.order, p.product_id))
    client.place_orders(PlaceOrdersParams(orders=[PlaceOrderParams(**b) for b in batch], stop_on_failure=True))
    out["place_orders"].append(
        {
            "network": "mainnet",
            "stop_on_failure": True,
            "orders": [
                {
                    "product_id": b["product_id"],
                    "id": b.get("id"),
                    "sender": subaccount_to_hex(b["order"].sender),
                    "price_x18": str(b["order"].priceX18),
                    "amount": str(b["order"].amount),
                    "expiration": str(b["order"].expiration),
                    "nonce": str(b["order"].nonce),
                    "appendix": str(b["order"].appendix),
                    "digest": d,
                }
                for b, d in zip(batch, digests)
            ],
            "request": captured[0],
        }
    )

    # cancel_orders
    d1 = out["place_order"][0]["digest"]
    d2 = out["place_order"][1]["digest"]
    cancel_cases = [
        ("mainnet", default_sub, [2, 4], [d1, d2], (1794780002000 << 20) + 5),
        ("mainnet", default_sub, [2], [d1], (1794780002000 << 20) + 6),
        ("testnet", bot_sub, [2], [d1], 1),
    ]
    for network, sender, pids, digs, nonce in cancel_cases:
        client, captured = client_for(key, network)
        p = CancelOrdersParams(sender=sender, productIds=pids, digests=digs, nonce=nonce)
        msg = client.prepare_execute_params(CancelOrdersParams.model_validate(p), True).dict()
        digest = client.build_digest(NadoExecuteType.CANCEL_ORDERS, msg, client.endpoint_addr, client.chain_id)
        client.cancel_orders(p)
        out["cancel_orders"].append(
            {
                "network": network,
                "sender": sender,
                "product_ids": pids,
                "digests": digs,
                "nonce": str(nonce),
                "digest": digest,
                "request": captured[0],
            }
        )

    # cancel_product_orders
    for network, sender, pids, nonce in [
        ("mainnet", default_sub, [2, 4], (1794780003000 << 20) + 9),
        ("mainnet", default_sub, [], (1794780003000 << 20) + 10),
        ("testnet", default_sub, [1], 42),
    ]:
        client, captured = client_for(key, network)
        p = CancelProductOrdersParams(sender=sender, productIds=pids, nonce=nonce)
        msg = client.prepare_execute_params(CancelProductOrdersParams.model_validate(p), True).dict()
        digest = client.build_digest(NadoExecuteType.CANCEL_PRODUCT_ORDERS, msg, client.endpoint_addr, client.chain_id)
        client.cancel_product_orders(p)
        out["cancel_product_orders"].append(
            {
                "network": network,
                "sender": sender,
                "product_ids": pids,
                "nonce": str(nonce),
                "digest": digest,
                "request": captured[0],
            }
        )

    # cancel_and_place
    client, captured = client_for(key, "mainnet")
    cap_order = dict(id=55, product_id=2, order=order(default_sub, 64_950 * 10**18, 3 * 10**16, 4294967295, (1794780004000 << 20) + 3, build_appendix(OrderType.POST_ONLY)))
    p_place = PlaceOrderParams(**cap_order)
    place_digest = client.get_order_digest(p_place.order, p_place.product_id)
    cancel_nonce = (1794780004000 << 20) + 4
    p_cancel = CancelOrdersParams(sender=default_sub, productIds=[2], digests=[d1], nonce=cancel_nonce)
    cancel_msg = client.prepare_execute_params(CancelOrdersParams.model_validate(p_cancel), True).dict()
    cancel_digest = client.build_digest(NadoExecuteType.CANCEL_ORDERS, cancel_msg, client.endpoint_addr, client.chain_id)
    client.cancel_and_place(CancelAndPlaceParams(cancel_orders=p_cancel, place_order=p_place))
    o = cap_order["order"]
    out["cancel_and_place"].append(
        {
            "network": "mainnet",
            "cancel": {"sender": default_sub, "product_ids": [2], "digests": [d1], "nonce": str(cancel_nonce), "digest": cancel_digest},
            "place": {
                "product_id": 2,
                "id": 55,
                "sender": default_sub,
                "price_x18": str(o.priceX18),
                "amount": str(o.amount),
                "expiration": str(o.expiration),
                "nonce": str(o.nonce),
                "appendix": str(o.appendix),
                "digest": place_digest,
            },
            "request": captured[0],
        }
    )

    # link_signer
    for network, signer_sub, nonce in [
        ("mainnet", subaccount_to_hex(other.address, "default"), 3),
        ("mainnet", "0x" + "00" * 32, 4),
        ("testnet", subaccount_to_hex(other.address, "default"), 0),
    ]:
        client, captured = client_for(key, network)
        p = LinkSignerParams(sender=default_sub, signer=signer_sub, nonce=nonce)
        msg = client.prepare_execute_params(LinkSignerParams.model_validate(p), False).dict()
        digest = client.build_digest(NadoExecuteType.LINK_SIGNER, msg, client.endpoint_addr, client.chain_id)
        client.link_signer(p)
        out["link_signer"].append(
            {
                "network": network,
                "sender": default_sub,
                "signer": signer_sub,
                "nonce": str(nonce),
                "digest": digest,
                "request": captured[0],
            }
        )

    # stream authentication
    for network, sender, expiration, msg_id in [
        ("mainnet", default_sub, 1794780060000, 1),
        ("testnet", bot_sub, 1794780099999, 42),
    ]:
        chain_id, endpoint = NETWORKS[network]
        msg = {"sender": subaccount_to_bytes32(sender), "expiration": expiration}
        typed = build_eip712_typed_data(NadoTxType.AUTHENTICATE_STREAM, msg, endpoint, chain_id)
        digest = get_eip712_typed_data_digest(typed)
        signature = sign_eip712_typed_data(typed, acct)
        auth = StreamAuthenticationParams(sender=sender, expiration=expiration, signature=signature)
        out["stream_auth"].append(
            {
                "network": network,
                "sender": sender,
                "expiration": str(expiration),
                "id": msg_id,
                "digest": digest,
                "message": messages.authenticate_message(auth, msg_id),
            }
        )

    json.dump(out, sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
