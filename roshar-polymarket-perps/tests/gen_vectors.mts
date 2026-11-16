// Generates tests/vectors.json from the official Polymarket TypeScript SDK
// (github.com/Polymarket/ts-sdk, packages/client = @polymarket/client).
//
// The Perps signing helpers the SDK uses internally (signPerpsOp,
// createPerpsOpTypedDataPayload, toPerpsCommandBodyOp, the trading actions and
// openPerpsSession / revokePerpsCredentials) are not exported from the npm
// bundle, so this script imports them from the SDK source tree.
//
// Run, with no network access needed:
//   git clone https://github.com/Polymarket/ts-sdk /tmp/ts-sdk
//   cd /tmp/ts-sdk && git checkout 3f0d9b85532d745b513596f4715aa49647acea1d && pnpm install
//   pnpm --filter @polymarket/types --filter @polymarket/bindings run build
//   cd /tmp/ts-sdk && SDK_DIR=/tmp/ts-sdk npx tsx <repo>/roshar-polymarket-perps/tests/gen_vectors.mts \
//     > <repo>/roshar-polymarket-perps/tests/vectors.json
//
// Keys are fresh throwaway keys from viem's generatePrivateKey on every run.
// Never regenerate the vectors to make a change pass.
//
// The trading actions take an executor. The executor below does what
// PerpsSession.executeCommand / executeDeleteCommand and #createSignedCommand do
// (session.ts), with a fixed salt and timestamp instead of randomUint32() and
// Date.now(): it signs the SDK-built compact op with the SDK's signPerpsOp and
// builds the body with the SDK's toPerpsCommandBodyOp. The WebSocket frame is
// `{...command, id, op, req: 'post'}` and the REST body is `{...command, op}`,
// as in session.ts.

const SDK = process.env.SDK_DIR;
if (!SDK) throw new Error('set SDK_DIR to a ts-sdk checkout');
const src = `${SDK}/packages/client/src`;
const mods = `${SDK}/packages/client/node_modules`;

// Dynamic imports: the SDK checkout location is chosen at run time (SDK_DIR) and
// the script lives outside the SDK tree, so static specifiers cannot resolve it.
const signing = await import(`${src}/websockets/perps/signing.ts`);
const trading = await import(`${src}/websockets/perps/actions/trading.ts`);
const perpsActions = await import(`${src}/actions/perps.ts`);
const { privateKey: viemPrivateKeySigner } = await import(`${src}/viem.ts`);
const { encode } = await import(`${mods}/@msgpack/msgpack/dist.esm/index.mjs`);
const TypedData = await import(`${mods}/ox/_esm/core/TypedData.js`);
const Hash = await import(`${mods}/ox/_esm/core/Hash.js`);
const { generatePrivateKey, privateKeyToAccount } = await import(
  `${mods}/viem/_esm/accounts/index.js`
);

const POLYGON = 137;
const AMOY = 80002;

const proxyKey = generatePrivateKey();
const proxyAddress = privateKeyToAccount(proxyKey).address;
const ownerKey = generatePrivateKey();
const ownerAddress = privateKeyToAccount(ownerKey).address;

const toHex = (bytes: Uint8Array) =>
  `0x${Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('')}`;

function compact(value: unknown): unknown {
  return Array.isArray(value)
    ? value.filter((v) => v !== undefined).map(compact)
    : value;
}

function opHashes(op: unknown, chainId: number, salt: number, ts: number) {
  const packed = encode(compact(op));
  const payload = signing.createPerpsOpTypedDataPayload({
    chainId,
    op,
    salt,
    timestamp: ts,
  });
  return {
    msgpack: toHex(packed),
    data: Hash.keccak256(packed, { as: 'Hex' }),
    digest: TypedData.getSignPayload(payload),
  };
}

type Captured = Record<string, unknown>;

interface Schema {
  parse(value: unknown): unknown;
}

interface CommandRequest {
  op: unknown;
  expiresAt?: number;
}

/** Stand-ins for the PerpsSession surfaces the SDK trading actions call. */
interface Executor {
  captured: Captured;
  executor: {
    builderAttribution: undefined;
    executeCommand(request: CommandRequest, schema: Schema): Promise<unknown>;
    executeDeleteCommand(request: CommandRequest & { path: string; responseSchema: Schema }): Promise<unknown>;
  };
  serviceClient: {
    patch(path: string, options: { json: Record<string, unknown>; schema: Schema }): Promise<unknown>;
  };
  signCommand(op: unknown, expiresAt?: number): Record<string, unknown>;
}

function executor(chainId: number, salt: number, ts: number, id: number, response: unknown): Executor {
  const captured: Captured = {};
  const sign = (op: unknown, expiresAt?: number) => {
    const sig = signing.signPerpsOp({ chainId, op, privateKey: proxyKey, salt, timestamp: ts });
    const command: Record<string, unknown> = { salt, sig, ts };
    if (expiresAt !== undefined) command.exp = expiresAt;
    return command;
  };
  const record = (op: unknown, expiresAt: number | undefined, path?: string) => {
    const command = sign(op, expiresAt);
    const bodyOp = trading.toPerpsCommandBodyOp(op);
    Object.assign(captured, {
      chain_id: chainId,
      salt,
      ts,
      exp: expiresAt ?? null,
      op,
      ...opHashes(op, chainId, salt, ts),
      sig: command.sig,
      rest_body: JSON.stringify({ ...command, op: bodyOp }),
      ws_id: id,
      ws_frame: JSON.stringify({ ...command, id, op: bodyOp, req: 'post' }),
    });
    if (path !== undefined) captured.path = path;
  };
  return {
    captured,
    executor: {
      builderAttribution: undefined,
      async executeCommand(request: CommandRequest, schema: Schema) {
        record(request.op, request.expiresAt);
        return schema.parse(response);
      },
      async executeDeleteCommand(request: CommandRequest & { path: string; responseSchema: Schema }) {
        record(request.op, request.expiresAt, request.path);
        return request.responseSchema.parse(response);
      },
    },
    // ServiceClient + signCommand pair used by the auto-cancel actions.
    serviceClient: {
      async patch(path: string, options: { json: Record<string, unknown>; schema: Schema }) {
        const { op: bodyOp, ...command } = options.json;
        captured.path = path;
        captured.rest_body = JSON.stringify({ ...command, op: bodyOp });
        return options.schema.parse(response);
      },
    },
    signCommand(op: unknown, expiresAt?: number) {
      record(op, expiresAt);
      return sign(op, expiresAt);
    },
  };
}

const vectors: Captured[] = [];
let wsId = 1;

async function command(
  name: string,
  input: unknown,
  response: unknown,
  run: (ex: Executor) => Promise<unknown>,
  opts: { salt: number; ts: number; chainId?: number },
) {
  const ex = executor(opts.chainId ?? POLYGON, opts.salt, opts.ts, wsId++, response);
  await run(ex);
  vectors.push({ name, input, ...ex.captured });
}

const ack = (n: number) => Array.from({ length: n }, (_, i) => ({ status: 'ok', oid: 1000 + i }));
const coid = '7f9e4a2b6c8d0e1f1234567890abcdef';
const coid2 = '00000000000000000000000000000001';

// createOrders, via postPerpsOrders (the SDK's toRawPerpsOrder builds the compact op).
const orderCases: Array<{ name: string; orders: Record<string, unknown>[]; expiresAt?: number; salt: number; ts: number; chainId?: number }> = [
  {
    name: 'gtc_post_only_coid',
    orders: [{ instrumentId: 1, side: 'BUY', price: '65000', quantity: '0.01', timeInForce: 'gtc', postOnly: true, clientOrderId: coid }],
    salt: 234567890, ts: 1767000010000,
  },
  {
    name: 'gtc_sell_plain',
    orders: [{ instrumentId: 6, side: 'SELL', price: '64999.5', quantity: '1.25', timeInForce: 'gtc' }],
    salt: 0, ts: 1794780000000,
  },
  {
    name: 'ioc_limit_reduce_only',
    orders: [{ instrumentId: 300, side: 'SELL', price: '0.12345', quantity: '100000', timeInForce: 'ioc', reduceOnly: true }],
    salt: 4294967295, ts: 1794780000001,
  },
  {
    name: 'ioc_market_no_price',
    orders: [{ instrumentId: 7, side: 'BUY', quantity: '2', timeInForce: 'ioc', clientOrderId: coid2 }],
    salt: 127, ts: 1794780000002,
  },
  {
    name: 'fok_limit',
    orders: [{ instrumentId: 65535, side: 'BUY', price: '3000.1', quantity: '0.5', timeInForce: 'fok' }],
    salt: 128, ts: 1794780000003,
  },
  {
    name: 'gtd_with_command_expiry',
    orders: [{ instrumentId: 65536, side: 'SELL', price: '101', quantity: '3', timeInForce: 'gtd', gtdExpiry: 1900000000000, postOnly: true, reduceOnly: true, clientOrderId: coid }],
    expiresAt: 1794780005000, salt: 65535, ts: 1794780000004,
  },
  {
    name: 'batch_three',
    orders: [
      { instrumentId: 1, side: 'BUY', price: '100.25', quantity: '7', timeInForce: 'gtc', postOnly: true, clientOrderId: coid },
      { instrumentId: 1, side: 'SELL', price: '101.75', quantity: '7', timeInForce: 'gtc', postOnly: true, clientOrderId: coid2 },
      { instrumentId: 2, side: 'BUY', quantity: '0.001', timeInForce: 'fok', reduceOnly: true },
    ],
    salt: 65536, ts: 1794780000005,
  },
  {
    name: 'amoy_chain',
    orders: [{ instrumentId: 1, side: 'BUY', price: '1', quantity: '1', timeInForce: 'gtc' }],
    salt: 42, ts: 1794780000006, chainId: AMOY,
  },
];
for (const c of orderCases) {
  await command(
    `create_orders/${c.name}`,
    { orders: c.orders, expires_at: c.expiresAt ?? null },
    ack(c.orders.length),
    (ex) => trading.postPerpsOrders(ex.executor, { orders: c.orders, expiresAt: c.expiresAt }),
    c,
  );
}

// cancelOrders / cancelOrdersCOID.
await command('cancel_orders/one', { order_ids: [1234567890], expires_at: null },
  [{ status: 'ok', oid: 1234567890, ts: 1 }],
  (ex) => trading.cancelPerpsOrders(ex.executor, { orderIds: [1234567890] }),
  { salt: 234567892, ts: 1767000030000 });
await command('cancel_orders/many_with_expiry', { order_ids: [1, 255, 9007199254740991], expires_at: 1794780009000 },
  [1, 255, 9007199254740991].map((oid) => ({ status: 'ok', oid, ts: 1 })),
  (ex) => trading.cancelPerpsOrders(ex.executor, { orderIds: [1, 255, 9007199254740991], expiresAt: 1794780009000 }),
  { salt: 99, ts: 1794780000010 });
await command('cancel_orders_coid/two', { client_order_ids: [coid, coid2], expires_at: null },
  [{ status: 'ok', oid: 1, coid, ts: 1 }, { status: 'ok', oid: 2, coid: coid2, ts: 1 }],
  (ex) => trading.cancelPerpsOrders(ex.executor, { clientOrderIds: [coid, coid2] }),
  { salt: 234567893, ts: 1767000040000 });

// cancelAll (REST DELETE in the SDK).
await command('cancel_all/all', { instrument_id: null, expires_at: null }, { status: 'ok' },
  (ex) => trading.cancelAllOrders(ex.executor, {}),
  { salt: 234567894, ts: 1767000050000 });
await command('cancel_all/one_instrument_with_expiry', { instrument_id: 1, expires_at: 1794780009000 }, { status: 'ok' },
  (ex) => trading.cancelAllOrders(ex.executor, { instrumentId: 1, expiresAt: 1794780009000 }),
  { salt: 234567895, ts: 1767000060000 });

// autoCancel (REST PATCH in the SDK).
await command('auto_cancel/arm', { time: 1900000000000 }, { status: 'ok', deadline: 1900000000000 },
  (ex) => trading.armPerpsAutoCancel(ex.serviceClient, ex.signCommand, { cancelAt: 1900000000000 }),
  { salt: 234567896, ts: 1767000070000 });
await command('auto_cancel/disarm', { time: 0 }, { status: 'ok', deadline: 0 },
  (ex) => trading.disarmPerpsAutoCancel(ex.serviceClient, ex.signCommand, {}),
  { salt: 234567897, ts: 1767000080000 });

// Leverage and isolated margin.
await command('update_leverage/cross', { instrument_id: 6, leverage: 20, cross: true },
  { status: 'ok', instrument_id: 6, leverage: 20, cross: true },
  (ex) => trading.updatePerpsLeverage(ex.executor, { instrumentId: 6, leverage: 20, crossMargin: true }),
  { salt: 5, ts: 1794780000020 });
await command('update_leverages/batch', { updates: [[1, 50, false], [6, 3, true]] },
  [{ status: 'ok', instrument_id: 1, leverage: 50, cross: false }, { status: 'ok', instrument_id: 6, leverage: 3, cross: true }],
  (ex) => trading.updatePerpsLeverages(ex.executor, { updates: [{ instrumentId: 1, leverage: 50, crossMargin: false }, { instrumentId: 6, leverage: 3, crossMargin: true }] }),
  { salt: 6, ts: 1794780000021 });
await command('update_margin/remove', { instrument_id: 6, amount: '-12.5' }, { status: 'ok' },
  (ex) => trading.updatePerpsMargin(ex.executor, { instrumentId: 6, amount: '-12.5' }),
  { salt: 7, ts: 1794780000022 });

// modifyOrders / modifyOrdersCOID: not implemented by the SDK yet ("SDK support is
// coming soon"). The compact ops are the documented ones
// (docs.polymarket.com/perps/trading.md#modify-an-order); hashing and signing use
// the SDK's createPerpsOpTypedDataPayload / signPerpsOp. Bodies follow the docs.
for (const m of [
  { name: 'modify_orders/one', type: 'modifyOrders', key: 'oid', items: [[1234567890, '100.25', '7']] as const, salt: 234567894, ts: 1767000050000 },
  { name: 'modify_orders/two_with_expiry', type: 'modifyOrders', key: 'oid', items: [[1, '0.5', '10'], [70000, '65000.1', '0.0001']] as const, salt: 8, ts: 1794780000030, exp: 1794780003000 },
  { name: 'modify_orders_coid/one', type: 'modifyOrdersCOID', key: 'coid', items: [[coid, '100.25', '7']] as const, salt: 9, ts: 1794780000031 },
]) {
  const op = [m.type, m.items.map((i) => [...i])];
  const sig = signing.signPerpsOp({ chainId: POLYGON, op, privateKey: proxyKey, salt: m.salt, timestamp: m.ts });
  const command: Record<string, unknown> = { salt: m.salt, sig, ts: m.ts };
  if (m.exp !== undefined) command.exp = m.exp;
  const bodyOp = { type: m.type, args: m.items.map(([id, p, qty]) => ({ [m.key]: id, p, qty })) };
  const id = wsId++;
  vectors.push({
    name: m.name,
    input: { items: m.items, expires_at: m.exp ?? null },
    chain_id: POLYGON, salt: m.salt, ts: m.ts, exp: m.exp ?? null, op,
    ...opHashes(op, POLYGON, m.salt, m.ts),
    sig,
    rest_body: JSON.stringify({ ...command, op: bodyOp }),
    ws_id: id,
    ws_frame: JSON.stringify({ ...command, id, op: bodyOp, req: 'post' }),
  });
}

// createProxy / deleteProxy, signed by the account owner through the SDK's viem
// signer, captured from openPerpsSession / revokePerpsCredentials with a stub client.
// Salt, timestamp and the generated proxy are the SDK's own random values.
const ownerSigner = viemPrivateKeySigner(ownerKey);
const ownerVectors: Captured[] = [];

/** Body the SDK sends to POST/DELETE /v1/account/proxy. */
interface OwnerBody {
  op: { type: string; args: { proxy: string; owner?: string; expiry?: number } };
  salt: number;
  ts: number;
  sig: string;
  label?: string;
}

interface OwnerCapture {
  path?: string;
  rest_body?: string;
  body?: OwnerBody;
}

function stubClient(capture: OwnerCapture) {
  return {
    account: { signer: ownerAddress },
    signer: ownerSigner,
    environment: { chainId: POLYGON },
    perps: {
      async post(path: string, { json }: { json: OwnerBody }) {
        capture.path = path;
        capture.rest_body = JSON.stringify(json);
        capture.body = json;
        return { secret: 'stub-secret' };
      },
      async del(path: string, { json }: { json: OwnerBody }) {
        capture.path = path;
        capture.rest_body = JSON.stringify(json);
        capture.body = json;
        return { status: 'ok' };
      },
      async getWorkflow() {
        const proxy = capture.body?.op.args.proxy ?? '';
        return { address: ownerAddress, keys: [{ proxy, expiresAt: Number.MAX_SAFE_INTEGER, label: undefined }] };
      },
    },
    webSockets: { perpsSession: { connect: async (credentials: unknown) => credentials } },
  };
}

function capturedBody(capture: OwnerCapture): OwnerBody {
  if (capture.body === undefined) throw new Error('SDK sent no request');
  return capture.body;
}

for (const label of [undefined, 'trading-app']) {
  const capture: OwnerCapture = {};
  await perpsActions.openPerpsSession(stubClient(capture), label === undefined ? {} : { label, expiresIn: 3_600_000 });
  const body = capturedBody(capture);
  const expiry = body.op.args.expiry;
  if (expiry === undefined) throw new Error('createProxy without expiry');
  const payload = {
    domain: { chainId: POLYGON, name: 'Polymarket', version: '1' },
    message: { addr: body.op.args.proxy, exp: expiry, salt: body.salt, ts: body.ts },
    primaryType: 'CreateProxy',
    types: {
      CreateProxy: [
        { name: 'addr', type: 'address' },
        { name: 'exp', type: 'uint64' },
        { name: 'salt', type: 'uint64' },
        { name: 'ts', type: 'uint64' },
      ],
      EIP712Domain: [
        { name: 'name', type: 'string' },
        { name: 'version', type: 'string' },
        { name: 'chainId', type: 'uint256' },
      ],
    },
  };
  const { path, rest_body } = capture;
  ownerVectors.push({
    name: label === undefined ? 'create_proxy/no_label' : 'create_proxy/label',
    chain_id: POLYGON,
    owner: ownerAddress,
    proxy: body.op.args.proxy,
    expiry,
    label: label ?? null,
    salt: body.salt,
    ts: body.ts,
    digest: TypedData.getSignPayload(payload),
    sig: body.sig,
    path,
    rest_body,
  });
}
{
  const capture: OwnerCapture = {};
  await perpsActions.revokePerpsCredentials(stubClient(capture), { proxy: proxyAddress });
  const body = capturedBody(capture);
  const op = ['deleteProxy', [proxyAddress]];
  const { path, rest_body } = capture;
  ownerVectors.push({
    name: 'delete_proxy',
    chain_id: POLYGON,
    proxy: proxyAddress,
    salt: body.salt,
    ts: body.ts,
    op,
    ...opHashes(op, POLYGON, body.salt, body.ts),
    sig: body.sig,
    path,
    rest_body,
  });
}

console.log(
  JSON.stringify(
    {
      generator: 'tests/gen_vectors.mts',
      sdk: '@polymarket/client 0.18.0 (github.com/Polymarket/ts-sdk 3f0d9b85532d745b513596f4715aa49647acea1d, packages/client/src)',
      proxy_private_key: proxyKey,
      proxy_address: proxyAddress,
      owner_private_key: ownerKey,
      owner_address: ownerAddress,
      commands: vectors,
      owner_ops: ownerVectors,
    },
    null,
    2,
  ),
);
