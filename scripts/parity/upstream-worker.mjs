// Composes a stack of IFCX files with upstream's TypeScript, bundled at run
// time from the checkout in IFCX_UPSTREAM_DIR (see ../upstream-parity.sh).
// Runs in a worker thread so the caller can give it a deep stack: upstream
// composes recursively.
//
// workerData: { bundle, files }. Posts the composed tree as a JSON string
// ({path, attributes, children}, like compose-json), or {error: {message}}.

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { parentPort, workerData } from "node:worker_threads";

const { LoadIfcxFile } = createRequire(import.meta.url)(workerData.bundle);

// Same as upstream's Federate for files without imports: data in layer
// order, weakest first. Schemas are not needed; validation is skipped.
const data = [];
for (const file of workerData.files) {
  for (const node of JSON.parse(readFileSync(file, "utf8")).data) data.push(node);
}
const federated = { header: {}, imports: [], schemas: {}, data };

// PostCompositionNode {node, attributes: Map, children: Map} to plain JSON,
// without recursion so deep trees do not need more stack than composing.
function toJson(root) {
  const out = {};
  const stack = [[root, out]];
  while (stack.length) {
    const [node, target] = stack.pop();
    target.path = node.node;
    target.attributes = {};
    for (const [id, value] of node.attributes) target.attributes[id] = value;
    target.children = {};
    for (const [name, child] of node.children) {
      const slot = {};
      target.children[name] = slot;
      stack.push([child, slot]);
    }
  }
  return out;
}

let result;
try {
  result = JSON.stringify(toJson(LoadIfcxFile(federated, false, true)));
} catch (e) {
  result = JSON.stringify({ error: { message: String(e && e.message ? e.message : e) } });
}
parentPort.postMessage(result);
