// The packed package from Node, resolved by name through its `exports` map
// (#46): `require` and `import` both reach the CommonJS build, and the `web`
// build loads in Node when handed its module bytes. Run from a directory
// whose node_modules holds the unpacked tarball and whose fixtures/ holds
// the repository's fixtures; see tools/check-package.mjs.
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { smoke } from "./smoke-core.mjs";

import * as imported from "@openbim/ifcx";
import init, * as web from "@openbim/ifcx/web";

const required = createRequire(import.meta.url)("@openbim/ifcx");
for (const name of ["IfcxFile", "compose", "validate", "exportGlb", "fetchImports"]) {
  if (typeof imported[name] !== "function" || required[name] !== imported[name]) {
    throw new Error(`require and import of @openbim/ifcx disagree on ${name}`);
  }
}

const wasm = new URL("openbim_ifcx_wasm_bg.wasm", import.meta.resolve("@openbim/ifcx/web"));
await init({ module_or_path: await readFile(wasm) });

// Node's fetch has no file: URLs; a custom fetch function reads the files.
const base = new URL("../fixtures/", import.meta.url);
const host = {
  base,
  fetch: async (url) => readFile(fileURLToPath(url)),
  text: (name) => readFile(new URL(name, base), "utf8"),
};

export const result = { node: await smoke(imported, host), web: await smoke(web, host) };
