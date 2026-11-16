// Generates tests/vectors.json with @solana/kit, the library the official Jupiter CLI
// (github.com/jup-ag/cli, src/lib/Signer.ts) signs Perps API transactions with:
// getTransactionCodec().decode -> partiallySignTransactionWithSigners -> getBase64EncodedWireTransaction.
//
// No network. The key below is a throwaway, generated once with crypto.randomBytes(32) and used to
// request the recorded unsigned transactions in tests/fixtures/create_limit_order*.json. It holds
// no funds and must never hold any.
//
// Run (kit is installed outside the repo):
//   mkdir -p /tmp/jupvec && cd /tmp/jupvec && npm init -y && npm i @solana/kit@6.10.0
//   cp <crate>/tests/gen_vectors.mjs /tmp/jupvec/ && node /tmp/jupvec/gen_vectors.mjs <crate>/tests
import {
  AccountRole,
  address,
  appendTransactionMessageInstruction,
  compileTransaction,
  createKeyPairSignerFromPrivateKeyBytes,
  createTransactionMessage,
  getBase58Decoder,
  getBase64Codec,
  getBase64EncodedWireTransaction,
  getTransactionCodec,
  getTransactionEncoder,
  partiallySignTransactionWithSigners,
  pipe,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
} from "@solana/kit";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const testsDir = process.argv[2];
if (!testsDir) throw new Error("usage: node gen_vectors.mjs <crate>/tests");

const SEED_HEX = "4ea47f937ae0e1524303125e1a9c3f197fe6a81694d9249bf169c4dd5304c12b";
const signer = await createKeyPairSignerFromPrivateKeyBytes(Buffer.from(SEED_HEX, "hex"));

const b64 = getBase64Codec();
const b58 = getBase58Decoder();
const txCodec = getTransactionCodec();

const fixture = (name) =>
  JSON.parse(readFileSync(join(testsDir, "fixtures", name), "utf8")).serializedTxBase64;

async function sign(name, unsignedB64) {
  const decoded = txCodec.decode(b64.encode(unsignedB64));
  const signed = await partiallySignTransactionWithSigners([signer], decoded);
  return {
    name,
    unsigned_b64: unsignedB64,
    signed_b64: getBase64EncodedWireTransaction(signed),
    signature_b58: b58.decode(signed.signatures[signer.address]),
  };
}

// Kit-built transactions cover layouts the API did not return for this key: a legacy message and
// a v0 message where the key is a non-fee-payer signer (index 1).
const MEMO = address("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
const OTHER_FEE_PAYER = address("Ag28fGtwtpnqassHURUBsQ1WfiyaWWzDDNs4Q28qHRjv");
const lifetime = {
  blockhash: "5NsiAsAkH9MCYytfSMUNEFpYKNdf6boNAP67SyGH85ZW",
  lastValidBlockHeight: 447904571n,
};
function built(version, feePayer, accounts) {
  const msg = pipe(
    createTransactionMessage({ version }),
    (m) => setTransactionMessageFeePayer(feePayer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(lifetime, m),
    (m) =>
      appendTransactionMessageInstruction(
        { programAddress: MEMO, accounts, data: new TextEncoder().encode("roshar") },
        m
      )
  );
  const tx = compileTransaction(msg);
  return b64.decode(getTransactionEncoder().encode(tx));
}

const cases = [
  await sign("create_limit_order_eth_long", fixture("create_limit_order.json")),
  await sign("create_limit_order_btc_short", fixture("create_limit_order_2.json")),
  await sign("create_limit_order_sol_long", fixture("create_limit_order_3.json")),
  await sign("kit_legacy_fee_payer", built("legacy", signer.address, [])),
  await sign(
    "kit_v0_second_signer",
    built(0, OTHER_FEE_PAYER, [{ address: signer.address, role: AccountRole.READONLY_SIGNER }])
  ),
];

// Transactions this key is not a required signer of: kit refuses them.
const rejected = [];
for (const [name, file] of [
  ["update_tpsl_other_wallet", "update_tpsl.json"],
  ["cancel_tpsl_other_wallet", "cancel_tpsl.json"],
]) {
  const unsigned = fixture(file);
  let error = null;
  try {
    await sign(name, unsigned);
  } catch (e) {
    error = String(e.message ?? e);
  }
  if (error === null) throw new Error(`${name}: kit signed a transaction it is not a signer of`);
  rejected.push({ name, unsigned_b64: unsigned, kit_error: error });
}

const out = {
  generator: "tests/gen_vectors.mjs",
  sdk: "@solana/kit 6.10.0",
  seed_hex: SEED_HEX,
  address: signer.address,
  cases,
  rejected,
};
writeFileSync(join(testsDir, "vectors.json"), JSON.stringify(out, null, 1) + "\n");
console.log(`wrote ${cases.length} cases, ${rejected.length} rejected`);
