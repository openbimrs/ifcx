// What smoke-core.mjs needs from a browser page: fixtures served by
// tools/check-package.mjs under /fixtures/, and the page's own `fetch`.
export const base = new URL("/fixtures/", location.href);

export const host = {
  base,
  fetch: undefined,
  async text(name) {
    const response = await fetch(new URL(name, base));
    if (!response.ok) throw new Error(`fixture ${name}: HTTP ${response.status}`);
    return response.text();
  },
};
