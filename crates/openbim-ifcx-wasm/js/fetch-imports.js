// Import resolution over `fetch`, for the `imports` option of `compose`,
// `validate` and `exportGlb`.
//
// Plain JavaScript on purpose: the Rust crates perform no network access
// (ADR 0002), so the binding resolves imports in memory only and this
// module does the fetching on the JavaScript side. It reads each layer's
// `imports` list, fetches every named file (recursively, each URI once) and
// returns the files keyed by their exact import `uri`, which is what the
// in-memory `imports` option expects. Integrity values are checked later,
// by the binding, against the fetched bytes.
//
// This file is the ES module source. scripts/build-npm-pkg.sh copies it into
// the `web` and `bundler` builds and derives the CommonJS copy for the Node
// build from it, so it must stay self-contained: no imports, and every
// export a top-level `export async function` or `export function`.

/** The imports of an IFCX file, or none when it is not readable JSON. */
function importUris(input) {
  let text = input;
  if (typeof input !== "string") {
    text = new TextDecoder().decode(input);
  }
  let file;
  try {
    file = JSON.parse(text);
  } catch {
    // Not JSON: the binding call that receives it reports where.
    return [];
  }
  const imports = file !== null && typeof file === "object" ? file.imports : undefined;
  if (!Array.isArray(imports)) return [];
  return imports
    .map((entry) => (entry !== null && typeof entry === "object" ? entry.uri : undefined))
    .filter((uri) => typeof uri === "string");
}

function fetchError(message, cause) {
  const error = new Error(message, cause === undefined ? undefined : { cause });
  error.name = "IfcxError";
  error.code = "fetch";
  return error;
}

function isInput(value) {
  return typeof value === "string" || value instanceof Uint8Array;
}

/** The URL `uri` names, relative to `base`; `undefined` if it cannot be resolved. */
function resolveUrl(uri, base) {
  try {
    return base === undefined ? new URL(uri).href : new URL(uri, base).href;
  } catch {
    return undefined;
  }
}

/** What a fetch function returned, as an IFCX input. */
async function readResponse(response, uri, url) {
  if (isInput(response)) return response;
  if (response instanceof ArrayBuffer) return new Uint8Array(response);
  if (response !== null && typeof response === "object" && typeof response.arrayBuffer === "function") {
    if (response.ok === false) {
      const status = [response.status, response.statusText].filter(Boolean).join(" ");
      throw fetchError(`could not fetch import ${JSON.stringify(uri)} from ${url}: HTTP ${status}`);
    }
    return new Uint8Array(await response.arrayBuffer());
  }
  throw fetchError(
    `the fetch function returned neither a Response, a Uint8Array, an ArrayBuffer nor a string for ${JSON.stringify(uri)}`,
  );
}

/**
 * Fetch every file the layers import, recursively, keyed by the exact
 * import `uri`, for the `imports` option of `compose`, `validate` and
 * `exportGlb`.
 */
export async function fetchImports(layers, options = {}) {
  const list = Array.isArray(layers) ? layers : [layers];
  for (const layer of list) {
    if (!isInput(layer)) {
      throw fetchError("every layer must be a string or a Uint8Array");
    }
  }
  const fetcher = options.fetch ?? globalThis.fetch;
  if (typeof fetcher !== "function") {
    throw fetchError("no fetch function: pass options.fetch");
  }
  const custom = options.fetch !== undefined;
  const base =
    options.baseUrl !== undefined ? String(options.baseUrl) : globalThis.location?.href;
  const init = options.signal === undefined ? undefined : { signal: options.signal };

  const found = new Map();
  if (options.imports !== undefined) {
    const known = options.imports instanceof Map ? options.imports : Object.entries(options.imports);
    for (const [uri, file] of known) found.set(uri, file);
  }

  // Breadth first; each level's fetches run concurrently. `url` is where a
  // layer came from, so its relative imports resolve against it.
  let level = list.map((file) => ({ file, url: base }));
  for (const [, file] of found) level.push({ file, url: base });
  while (level.length > 0) {
    const pending = [];
    for (const { file, url: importer } of level) {
      for (const uri of importUris(file)) {
        if (found.has(uri)) continue;
        found.set(uri, undefined); // claimed: fetch each URI once
        const url = resolveUrl(uri, importer);
        if (url === undefined && !custom) {
          throw fetchError(
            `cannot resolve import ${JSON.stringify(uri)} to a URL: pass options.baseUrl`,
          );
        }
        pending.push(
          (async () => {
            let response;
            try {
              response = await fetcher(url ?? uri, init);
            } catch (error) {
              throw fetchError(
                `could not fetch import ${JSON.stringify(uri)} from ${url ?? uri}: ${error?.message ?? error}`,
                error,
              );
            }
            const fetched = await readResponse(response, uri, url ?? uri);
            found.set(uri, fetched);
            return { file: fetched, url: url ?? importer };
          })(),
        );
      }
    }
    level = await Promise.all(pending);
  }
  return found;
}
