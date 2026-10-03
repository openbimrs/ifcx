// Check the built @openbim/ifcx package as a consumer gets it (#46).
// Adapted from openbimrs/ifc's crates/openbim-ifc-wasm/tools/check-package.mjs.
//
//   node crates/openbim-ifcx-wasm/tools/check-package.mjs <package-dir>
//
// 1. `npm pack` the package and unpack the tarball into a scratch project's
//    node_modules, so the `files` list and the `exports` map are what is
//    under test, not the build directory.
// 2. Node: tests/js/web/node-entry.mjs resolves the package by name through
//    `require` and `import`, and loads the `web` build from its bytes.
// 3. Bundler: webpack bundles tests/js/web/bundler-entry.mjs for the browser
//    with `experiments.asyncWebAssembly`, the bundler target's contract.
// 4. Browser: headless Chrome loads one page that imports the `web` build
//    (tests/js/web/web-entry.mjs, through an import map) and the webpack
//    bundle; the page posts each smoke result back to this script. Both
//    parse, validate and compose the repository's fixtures (served under
//    /fixtures/), resolve imports with the page's `fetch` through
//    `fetchImports`, and export GLB (tests/js/web/smoke-core.mjs). An
//    uncaught error or rejection in the page fails the check.
//
// Chrome is found through $CHROME_BIN, then the usual names on PATH, then a
// Playwright download. GitHub's hosted runners ship Google Chrome, so CI
// needs no browser install. No browser fails the check; IFCX_SKIP_BROWSER=1
// skips step 4 with a warning instead.
import { execFileSync, spawn } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { cp, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import webpack from "webpack";

const here = path.dirname(fileURLToPath(import.meta.url));
const entries = path.resolve(here, "../tests/js/web");
const fixtures = path.resolve(here, "../../openbim-ifcx/tests/fixtures");
const pkg = path.resolve(process.argv[2] ?? path.join(here, "../pkg"));
const TIMEOUT_MS = 120_000;

const HARNESS = `<!doctype html>
<meta charset="utf-8">
<title>@openbim/ifcx browser smoke</title>
<script type="importmap">
{ "imports": { "@openbim/ifcx/web": "/node_modules/@openbim/ifcx/web/openbim_ifcx_wasm.js" } }
</script>
<script>
  addEventListener("error", (event) =>
    fetch("/result", { method: "POST", body: JSON.stringify({ page: { error: String(event.message) } }) }));
  addEventListener("unhandledrejection", (event) =>
    fetch("/result", { method: "POST", body: JSON.stringify({ page: { error: String(event.reason) } }) }));
</script>
<script type="module">
  const results = {};
  for (const [name, url] of [["web", "/src/web-entry.mjs"], ["bundler", "/dist/bundle.mjs"]]) {
    try {
      results[name] = { ok: (await import(url)).result };
    } catch (error) {
      results[name] = { error: String((error && error.stack) || error) };
    }
  }
  await fetch("/result", { method: "POST", body: JSON.stringify(results) });
</script>
`;

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".json": "application/json",
  ".ifcx": "application/json",
  ".wasm": "application/wasm",
};

const scratch = await mkdtemp(path.join(os.tmpdir(), "openbim-ifcx-pkg-"));
let failed = false;
try {
  await unpack(scratch);
  await cp(entries, path.join(scratch, "src"), { recursive: true });
  await cp(fixtures, path.join(scratch, "fixtures"), { recursive: true });

  const node = await import(pathToFileURL(path.join(scratch, "src/node-entry.mjs")).href);
  console.log("node (require, import, web):", JSON.stringify(node.result));

  await bundle(scratch);
  console.log("webpack: bundled src/bundler-entry.mjs");

  const chrome = findChrome();
  if (chrome === undefined) {
    if (!process.env.IFCX_SKIP_BROWSER) {
      throw new Error(
        "no Chrome or Chromium found; set CHROME_BIN, or IFCX_SKIP_BROWSER=1 to skip the browser checks",
      );
    }
    console.warn("warning: IFCX_SKIP_BROWSER set; browser checks NOT run");
  } else {
    const results = await inBrowser(chrome, scratch);
    for (const [name, outcome] of Object.entries(results)) {
      if (outcome.error !== undefined) {
        failed = true;
        console.error(`browser ${name}: FAILED\n${outcome.error}`);
      } else {
        console.log(`browser ${name}:`, JSON.stringify(outcome.ok));
      }
    }
    for (const name of ["web", "bundler"]) {
      if (!(name in results)) {
        failed = true;
        console.error(`browser ${name}: no result`);
      }
    }
  }
} finally {
  await rm(scratch, { recursive: true, force: true });
}
if (failed) process.exit(1);

/** Pack `pkg` as npm publishes it and unpack it as an install would. */
async function unpack(dir) {
  const [{ filename }] = JSON.parse(
    execFileSync("npm", ["pack", "--json", "--pack-destination", dir], {
      cwd: pkg,
      encoding: "utf8",
    }),
  );
  const scope = path.join(dir, "node_modules/@openbim");
  await mkdir(scope, { recursive: true });
  execFileSync("tar", ["-xzf", path.join(dir, filename), "-C", scope]);
  await cp(path.join(scope, "package"), path.join(scope, "ifcx"), { recursive: true });
  await rm(path.join(scope, "package"), { recursive: true });
}

/** webpack 5 bundles the bundler entry for the browser, as an ES module. */
async function bundle(dir) {
  const compiler = webpack({
    mode: "production",
    target: "web",
    context: dir,
    entry: { bundle: "./src/bundler-entry.mjs" },
    output: {
      path: path.join(dir, "dist"),
      filename: "[name].mjs",
      module: true,
      library: { type: "module" },
    },
    experiments: { asyncWebAssembly: true, outputModule: true },
    optimization: { minimize: false },
    performance: { hints: false },
  });
  const stats = await new Promise((resolve, reject) =>
    compiler.run((error, result) => (error ? reject(error) : resolve(result))),
  );
  await new Promise((resolve) => compiler.close(resolve));
  if (stats.hasErrors()) {
    throw new Error(`webpack failed:\n${stats.toString({ all: false, errors: true })}`);
  }
}

function findChrome() {
  if (process.env.CHROME_BIN) return process.env.CHROME_BIN;
  const names = ["google-chrome-stable", "google-chrome", "chromium", "chromium-browser", "chrome"];
  for (const dir of (process.env.PATH ?? "").split(path.delimiter)) {
    for (const name of names) {
      if (dir && existsSync(path.join(dir, name))) return path.join(dir, name);
    }
  }
  const mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
  if (existsSync(mac)) return mac;
  const playwright = path.join(os.homedir(), ".cache/ms-playwright");
  if (existsSync(playwright)) {
    for (const build of readdirSync(playwright).filter((d) => /^chromium-\d+$/.test(d)).sort().reverse()) {
      for (const sub of ["chrome-linux64", "chrome-linux"]) {
        const exe = path.join(playwright, build, sub, "chrome");
        if (existsSync(exe)) return exe;
      }
    }
  }
  return undefined;
}

/** Serve `dir` on localhost, open the harness in headless Chrome, await its post. */
async function inBrowser(chrome, dir) {
  let report;
  const posted = new Promise((resolve) => (report = resolve));
  const server = createServer(async (request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (request.method === "POST" && url.pathname === "/result") {
      let body = "";
      for await (const chunk of request) body += chunk;
      response.end();
      report(JSON.parse(body));
      return;
    }
    if (url.pathname === "/") {
      response.setHeader("content-type", "text/html");
      response.end(HARNESS);
      return;
    }
    const file = path.join(dir, path.normalize(decodeURIComponent(url.pathname)));
    if (!file.startsWith(dir + path.sep)) {
      response.statusCode = 403;
      response.end();
      return;
    }
    try {
      const body = await readFile(file);
      response.setHeader("content-type", TYPES[path.extname(file)] ?? "application/octet-stream");
      response.end(body);
    } catch {
      response.statusCode = 404;
      response.end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  const profile = await mkdtemp(path.join(os.tmpdir(), "openbim-ifcx-chrome-"));
  const browser = spawn(
    chrome,
    [
      "--headless=new",
      // Hosted CI runners restrict the user namespaces Chrome's sandbox
      // needs; the page is this script's own, served from localhost.
      "--no-sandbox",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      `--user-data-dir=${profile}`,
      `http://127.0.0.1:${port}/`,
    ],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  let log = "";
  browser.stderr.on("data", (chunk) => (log += chunk));
  const exited = new Promise((resolve) => {
    browser.on("exit", resolve);
    browser.on("error", (error) => resolve(error.message));
  });
  const early = exited.then((code) => {
    throw new Error(`Chrome exited (${code}) before the page reported:\n${log}`);
  });
  early.catch(() => {}); // the kill below settles it after the race is decided
  let timer;
  try {
    return await Promise.race([
      posted,
      early,
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`no result from the page within ${TIMEOUT_MS} ms:\n${log}`)),
          TIMEOUT_MS,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
    browser.kill();
    await exited;
    server.close();
    // Chrome's helper processes can outlive the main one and still write
    // into the profile, so a single rmdir may race them (ENOTEMPTY).
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  }
}
