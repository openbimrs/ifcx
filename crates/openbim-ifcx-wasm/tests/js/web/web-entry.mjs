// The `web` target in a browser, without a bundler (#46). Served next to the
// unpacked package with an import map for `@openbim/ifcx/web`; see
// tools/check-package.mjs.
import { host } from "./browser-host.mjs";
import { smoke } from "./smoke-core.mjs";

import init, * as ifcx from "@openbim/ifcx/web";

await init(); // fetches openbim_ifcx_wasm_bg.wasm from next to the module

export const result = await smoke(ifcx, host);
