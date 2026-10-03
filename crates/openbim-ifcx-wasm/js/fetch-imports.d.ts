import type { IfcxInput } from "./openbim_ifcx_wasm.js";

/** The part of a `fetch` `Response` that `fetchImports` reads. */
export interface FetchedResponse {
  ok?: boolean;
  status?: number;
  statusText?: string;
  arrayBuffer(): Promise<ArrayBuffer>;
}

/**
 * Loads one imported file. `fetch` itself fits; a custom function may also
 * return the file directly. `url` is the import `uri` resolved against the
 * importing file's URL (or `baseUrl`); when a relative `uri` has no base to
 * resolve against, a custom function receives the `uri` unchanged.
 */
export type ImportFetcher = (
  url: string,
  init?: { signal?: AbortSignal },
) => Promise<FetchedResponse | IfcxInput | ArrayBuffer>;

/** Options for `fetchImports`. */
export interface FetchImportsOptions {
  /**
   * URL that relative import URIs of the given layers resolve against.
   * Defaults to `location.href` in a browser. Imports of a fetched file
   * resolve against that file's URL.
   */
  baseUrl?: string | URL;
  /** Loads each file. Defaults to the global `fetch`. */
  fetch?: ImportFetcher;
  /** Files already at hand, keyed by import `uri`; never fetched again. */
  imports?: Record<string, IfcxInput> | Map<string, IfcxInput>;
  /** Passed to every fetch call. */
  signal?: AbortSignal;
}

/**
 * Fetch every file the layers import, recursively and each `uri` once, and
 * return them keyed by the exact import `uri`: the `imports` option of
 * `compose`, `validate` and `exportGlb`. Network access happens here, in
 * JavaScript; the WebAssembly module never fetches anything. A file that
 * cannot be fetched rejects with an `IfcxError` whose `code` is `"fetch"`.
 *
 * ```js
 * const imports = await fetchImports(layers, { baseUrl: "https://example.org/model/" });
 * const tree = compose(layers, { imports });
 * ```
 */
export function fetchImports(
  layers: IfcxInput | IfcxInput[],
  options?: FetchImportsOptions,
): Promise<Map<string, IfcxInput>>;
