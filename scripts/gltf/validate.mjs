// Validates .glb files with the Khronos glTF validator (npm `gltf-validator`).
// Usage: node validate.mjs FILE.glb...
// The validator is loaded from $GLTF_VALIDATOR_DIR/node_modules if set, else
// from scripts/gltf/node_modules. Prints one line per file with issue counts,
// then each error and warning and a count per info or hint code; exits 1 if any file has an error.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const base = process.env.GLTF_VALIDATOR_DIR ?? fileURLToPath(new URL(".", import.meta.url));
const validator = createRequire(join(base, "package.json"))("gltf-validator");

let failed = false;
for (const file of process.argv.slice(2)) {
  const report = await validator.validateBytes(new Uint8Array(readFileSync(file)), {
    uri: basename(file),
    maxIssues: 0,
  });
  const { numErrors, numWarnings, numInfos, numHints, messages } = report.issues;
  console.log(
    `${numErrors} errors, ${numWarnings} warnings, ${numInfos} infos, ${numHints} hints  ${file}`,
  );
  for (const m of messages.filter((m) => m.severity <= 1)) {
    console.log(`    ${m.severity === 0 ? "error" : "warning"} ${m.code} ${m.pointer ?? ""}: ${m.message}`);
  }
  const minor = {};
  for (const m of messages.filter((m) => m.severity > 1)) minor[m.code] = (minor[m.code] ?? 0) + 1;
  for (const [code, n] of Object.entries(minor)) console.log(`    ${n}× ${code}`);
  failed ||= numErrors > 0;
}
if (failed) process.exit(1);
