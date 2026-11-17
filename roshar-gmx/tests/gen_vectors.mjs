// Generates tests/vectors.json with the official GMX SDK (@gmx-io/sdk) and the viem it uses.
// Offline: it signs the captured `prepare` responses in tests/fixtures with a throwaway random key.
//
//   mkdir -p /tmp/gmx-sdk && cd /tmp/gmx-sdk && npm init -y && npm install @gmx-io/sdk@2.9.1
//   cd /tmp/gmx-sdk && node /path/to/roshar-gmx/tests/gen_vectors.mjs
//
// Modules resolve from the current directory, so run it from the directory holding node_modules.
import { createRequire } from "node:module";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(join(process.cwd(), "noop.js"));
const { PrivateKeySigner } = require("@gmx-io/sdk/v2");
const { signPreparedOrder } = require("@gmx-io/sdk/utils/orderTransactions");
const { hashTypedData, hashDomain, hashStruct, getTypesForEIP712Domain } = require("viem");
const { generatePrivateKey } = require("viem/accounts");

const here = fileURLToPath(new URL(".", import.meta.url));
const fixture = (name) => JSON.parse(readFileSync(join(here, "fixtures", name), "utf8"));
const pkgVersion = (name) =>
  JSON.parse(readFileSync(join(process.cwd(), "node_modules", name, "package.json"), "utf8")).version;
const sdkVersion = pkgVersion("@gmx-io/sdk");
const viemVersion = pkgVersion("viem");

const privateKey = generatePrivateKey();
const signer = new PrivateKeySigner(privateKey);

// PrivateKeySigner.signTypedData uses the first non-EIP712Domain key as the primary type.
const primaryTypeOf = (types) => Object.keys(types).filter((t) => t !== "EIP712Domain")[0];

function hashes(typedData) {
  const { domain, types, message } = typedData;
  const primaryType = primaryTypeOf(types);
  return {
    primaryType,
    domainSeparator: hashDomain({
      domain,
      types: { EIP712Domain: getTypesForEIP712Domain({ domain }), ...types },
    }),
    structHash: hashStruct({ data: message, primaryType, types }),
    digest: hashTypedData({ domain, types, primaryType, message }),
  };
}

const PREPARED = [
  "prepare_increase_limit.json",
  "prepare_increase_market.json",
  "prepare_decrease_market.json",
  "prepare_edit.json",
  "prepare_cancel.json",
  "prepare_cancel_all.json",
];

const orders = [];
for (const name of PREPARED) {
  const prepared = fixture(name);
  const typedData = prepared.payload.typedData;
  const chainId = Number(typedData.domain.chainId);
  // The SDK validates receivers against the signer and, for delegated signing, the main account.
  // The throwaway key is never the account in these captures, so pass the account explicitly.
  const account = typedData.message.createOrderParamsList[0]?.addresses.receiver ?? typedData.message.account;
  const signature = await signPreparedOrder(prepared, signer, chainId, account);
  orders.push({ fixture: name, chainId, account, ...hashes(typedData), signature });
}

// Validation parity: signPreparedOrder runs validateOrderTypedData before signing.
const base = fixture("prepare_increase_limit.json");
const baseAccount = base.payload.typedData.message.createOrderParamsList[0].addresses.receiver;
const foreign = "0x2222222222222222222222222222222222222222";
function mutate(fn) {
  const p = structuredClone(base);
  fn(p.payload.typedData);
  return p;
}
const validationCases = [
  { name: "valid_with_account", prepared: base, chainId: 42161, account: baseAccount },
  { name: "receiver_not_signer_without_account", prepared: base, chainId: 42161, account: undefined },
  { name: "wrong_chain_id", prepared: base, chainId: 43114, account: baseAccount },
  { name: "wrong_domain_name", prepared: mutate((t) => (t.domain.name = "Evil")), chainId: 42161, account: baseAccount },
  { name: "wrong_domain_version", prepared: mutate((t) => (t.domain.version = "2")), chainId: 42161, account: baseAccount },
  { name: "unknown_verifying_contract", prepared: mutate((t) => (t.domain.verifyingContract = foreign)), chainId: 42161, account: baseAccount },
  { name: "legacy_relay_router_from_docs", prepared: mutate((t) => (t.domain.verifyingContract = "0x5503b99308dB6923758F9A22d118207D633c4e87")), chainId: 42161, account: baseAccount },
  { name: "foreign_receiver", prepared: mutate((t) => (t.message.createOrderParamsList[0].addresses.receiver = foreign)), chainId: 42161, account: baseAccount },
  { name: "foreign_cancellation_receiver", prepared: mutate((t) => (t.message.createOrderParamsList[0].addresses.cancellationReceiver = foreign)), chainId: 42161, account: baseAccount },
  { name: "lowercase_verifying_contract", prepared: mutate((t) => (t.domain.verifyingContract = t.domain.verifyingContract.toLowerCase())), chainId: 42161, account: baseAccount },
];
const validation = [];
for (const c of validationCases) {
  let ok = true;
  try {
    await signPreparedOrder(c.prepared, signer, c.chainId, c.account);
  } catch {
    ok = false;
  }
  validation.push({ name: c.name, chainId: c.chainId, account: c.account ?? null, typedData: c.prepared.payload.typedData, ok });
}

// Generic EIP-712 coverage beyond the GMX batch types (strings, bytes, signed ints, fixed arrays).
const synthetic = {
  domain: { name: "Synthetic", version: "7", chainId: 43114, verifyingContract: "0xE9f7E49F7304D36Aae146B5Af7cE6f9421Dac2CF" },
  types: {
    Mail: [
      { name: "from", type: "Person" },
      { name: "to", type: "Person[]" },
      { name: "contents", type: "string" },
      { name: "data", type: "bytes" },
      { name: "delta", type: "int256" },
      { name: "small", type: "int8" },
      { name: "count", type: "uint8" },
      { name: "tag", type: "bytes4" },
      { name: "flags", type: "bool[2]" },
      { name: "nonce", type: "uint256" },
    ],
    Person: [
      { name: "name", type: "string" },
      { name: "wallet", type: "address" },
    ],
  },
  message: {
    from: { name: "Cow", wallet: "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826" },
    to: [
      { name: "Bob", wallet: "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB" },
      { name: "Ünïcode ✓", wallet: "0x0000000000000000000000000000000000000001" },
    ],
    contents: "Hello, Bob!",
    data: "0xdeadbeef00",
    delta: "-123456789012345678901234567890",
    small: -128,
    count: 255,
    tag: "0x01020304",
    flags: [true, false],
    nonce: "115792089237316195423570985008687907853269984665640564039457584007913129639935",
  },
};
const syntheticVector = {
  typedData: synthetic,
  ...hashes(synthetic),
  signature: await signer.signTypedData(synthetic.domain, synthetic.types, synthetic.message),
};

const out = {
  generator: `@gmx-io/sdk ${sdkVersion} (signPreparedOrder, PrivateKeySigner), viem ${viemVersion}`,
  privateKey,
  address: signer.address,
  orders,
  validation,
  synthetic: syntheticVector,
};
writeFileSync(join(here, "vectors.json"), JSON.stringify(out, null, 1) + "\n");
console.log(`wrote ${orders.length} order vectors, ${validation.length} validation cases`);
