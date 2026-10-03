// The demo uses @openbim/ifcx as built from this repository's source, not
// from npm: crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh writes the
// package to crates/openbim-ifcx-wasm/pkg (or IFCX_WASM_PKG), and the alias
// below points the `web` build's import there. So the demo always shows the
// code on its branch and never waits for a publish.
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));
const pkg = path.resolve(here, process.env.IFCX_WASM_PKG ?? "../crates/openbim-ifcx-wasm/pkg");
const web = path.join(pkg, "web/openbim_ifcx_wasm.js");
if (!existsSync(web)) {
  throw new Error(
    `${web} is missing: build the package first with crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh`,
  );
}

export default defineConfig({
  // Relative asset URLs: the site is served below /ifcx/ on GitHub Pages.
  base: "./",
  resolve: {
    alias: [{ find: /^@openbim\/ifcx\/web$/, replacement: web }],
  },
  // The wasm glue finds its module through `new URL(..., import.meta.url)`,
  // which pre-bundling would break.
  optimizeDeps: { exclude: ["@openbim/ifcx/web"] },
  // The samples are the repository's own fixtures, outside this directory.
  server: { fs: { allow: [here, path.resolve(here, "../crates/openbim-ifcx/tests/fixtures"), pkg] } },
  build: {
    target: "es2022",
    // three.js and the wasm glue make one ~650 kB chunk; that is expected.
    chunkSizeWarningLimit: 1024,
    // Serve the sample files as files, never inlined as data: URLs, so a
    // sample's relative imports resolve against its own URL.
    assetsInlineLimit: (file) => (file.endsWith(".ifcx") ? false : undefined),
  },
});
