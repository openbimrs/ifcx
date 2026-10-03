// Compares this crate's composition with upstream's TypeScript composition,
// case by case. Run through ../upstream-parity.sh, which bundles upstream and
// builds the compose-json example first.
//
// usage: node parity.mjs <upstream bundle> <compose-json binary> <upstream dir> <fixtures dir>
//
// Cases:
// - every `.ifcx` file under <upstream dir>/examples, composed alone. A file
//   this crate rejects with an unknown reference is an overlay on other files
//   of its example folder (`Tunnel Excavation/02_*` over `01_*`); it is
//   composed again, by both sides, on top of every other `.ifcx` file of the
//   folder directly below `examples/`, in sorted order, itself last.
// - this crate's hand-written fixtures and the layer stacks they form.
//
// Trees are compared as JSON values, with -0 read as 0 (JavaScript writes
// -0 as 0) and object key order ignored; key order is reported separately
// (upstream's JavaScript objects list integer-like keys first). A case
// matches when both trees are equal or both sides reject the input. A case
// where only this crate rejects an unknown reference, which upstream composes
// as an empty node, is reported as a known divergence. Exit code 1 if any
// case differs.

import { spawnSync } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { Worker } from "node:worker_threads";

const [bundle, rustBin, upstreamDir, fixturesDir] = process.argv.slice(2);
if (!fixturesDir) {
  console.error("usage: node parity.mjs <upstream bundle> <compose-json> <upstream dir> <fixtures dir>");
  process.exit(2);
}
const stackSizeMb = Number(process.env.IFCX_PARITY_STACK_MB || 1024);

function ifcxFiles(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (name !== ".git" && name !== "node_modules") out.push(...ifcxFiles(path));
    } else if (name.endsWith(".ifcx")) {
      out.push(path);
    }
  }
  return out.sort();
}

function ours(files) {
  const run = spawnSync(rustBin, files, { maxBuffer: 1 << 29, encoding: "utf8" });
  if (run.status !== 0) throw new Error(`compose-json ${files.join(" ")}: ${run.stderr}`);
  return JSON.parse(run.stdout);
}

function upstream(files) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./upstream-worker.mjs", import.meta.url), {
      workerData: { bundle, files },
      resourceLimits: { stackSizeMb, maxOldGenerationSizeMb: 8192 },
    });
    worker.once("message", (json) => resolve(JSON.parse(json)));
    worker.once("error", reject);
    worker.once("exit", (code) => code !== 0 && reject(new Error(`worker exit ${code}`)));
  });
}

// Replaces -0 by 0 in place; returns how many were replaced.
function normaliseZero(value) {
  let count = 0;
  const stack = [value];
  while (stack.length) {
    const v = stack.pop();
    if (v === null || typeof v !== "object") continue;
    for (const k of Object.keys(v)) {
      if (Object.is(v[k], -0)) {
        v[k] = 0;
        count++;
      } else if (typeof v[k] === "object") {
        stack.push(v[k]);
      }
    }
  }
  return count;
}

const isObject = (v) => v !== null && typeof v === "object" && !Array.isArray(v);

// First difference as a JSON-pointer-like path, ignoring object key order;
// null if equal. `order` collects the first place key order differs.
function firstDiff(a, b, order) {
  const stack = [[a, b, ""]];
  while (stack.length) {
    const [x, y, at] = stack.pop();
    if (Array.isArray(x) && Array.isArray(y)) {
      if (x.length !== y.length) return `${at}: length ${x.length} here, ${y.length} upstream`;
      for (let i = x.length - 1; i >= 0; i--) stack.push([x[i], y[i], `${at}/${i}`]);
    } else if (isObject(x) && isObject(y)) {
      const kx = Object.keys(x);
      const ky = Object.keys(y);
      for (const k of kx) if (!Object.hasOwn(y, k)) return `${at}/${k}: only here`;
      for (const k of ky) if (!Object.hasOwn(x, k)) return `${at}/${k}: only upstream`;
      if (!order.at && kx.some((k, i) => k !== ky[i])) order.at = at || "/";
      for (let i = kx.length - 1; i >= 0; i--) stack.push([x[kx[i]], y[kx[i]], `${at}/${kx[i]}`]);
    } else if (x !== y) {
      const show = (v) => JSON.stringify(v).slice(0, 60);
      return `${at}: ${show(x)} here, ${show(y)} upstream`;
    }
  }
  return null;
}

async function check(name, files) {
  const mine = ours(files);
  const theirs = await upstream(files);
  if (mine.error && theirs.error) return { status: "MATCH", note: `both reject (${mine.error.kind})` };
  if (mine.error?.kind === "unknown-reference" && !theirs.error) {
    return { status: "KNOWN", note: `${mine.error.message}; upstream composes an empty node` };
  }
  if (mine.error || theirs.error) {
    return { status: "DIFF", note: `here: ${mine.error?.message ?? "tree"}; upstream: ${theirs.error?.message ?? "tree"}` };
  }
  const zeros = normaliseZero(mine);
  const order = {};
  const diff = firstDiff(mine, theirs, order);
  if (diff) return { status: "DIFF", note: diff };
  const notes = [];
  if (zeros) notes.push(`${zeros} × -0 written as 0 upstream`);
  if (order.at) notes.push(`key order differs at ${order.at}`);
  return { status: "MATCH", note: notes.join("; ") };
}

const cases = [];
const examples = join(upstreamDir, "examples");
for (const file of ifcxFiles(examples)) {
  const name = relative(examples, file);
  let files = [file];
  const alone = ours(files);
  if (alone.error?.kind === "unknown-reference") {
    const folder = join(examples, name.split(sep)[0]);
    files = ifcxFiles(folder).filter((f) => f !== file).concat(file);
  }
  cases.push({ name: `examples/${name}`, files });
}
const fixture = (...names) => names.map((n) => join(fixturesDir, n));
for (const name of readdirSync(fixturesDir).filter((n) => n.endsWith(".ifcx")).sort()) {
  cases.push({ name: `fixtures/${name}`, files: fixture(name) });
}
cases.push({ name: "fixtures/layer-base.ifcx + layer-edit.ifcx", files: fixture("layer-base.ifcx", "layer-edit.ifcx") });
// Upstream layer order for main → mid → sub/base: the main layer first.
cases.push({
  name: "fixtures/layers/chain (main, mid, sub/base)",
  files: fixture("layers/chain/main.ifcx", "layers/chain/mid.ifcx", "layers/chain/sub/base.ifcx"),
});

const counts = { MATCH: 0, KNOWN: 0, DIFF: 0 };
console.log("| Case | Layers | Result | Notes |\n| --- | --- | --- | --- |");
for (const { name, files } of cases) {
  let result;
  try {
    result = await check(name, files);
  } catch (e) {
    result = { status: "DIFF", note: `failed to run: ${e.message}` };
  }
  counts[result.status]++;
  console.log(`| ${name} | ${files.length} | ${result.status} | ${result.note} |`);
}
console.log(`\n${cases.length} cases: ${counts.MATCH} match, ${counts.KNOWN} known divergences, ${counts.DIFF} differ`);
process.exit(counts.DIFF ? 1 : 0);
