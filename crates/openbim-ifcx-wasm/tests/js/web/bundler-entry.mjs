// The package's default ES module export through a bundler (#46). Bundled by
// webpack with `experiments.asyncWebAssembly` for the browser, then run in
// the same headless browser page as the `web` target; see
// tools/check-package.mjs.
import { host } from "./browser-host.mjs";
import { smoke } from "./smoke-core.mjs";

import * as ifcx from "@openbim/ifcx"; // the bundler loads the wasm module

export const result = await smoke(ifcx, host);
