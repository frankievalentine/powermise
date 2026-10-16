// Which links the app opens in the browser: the hosts below, and homepages
// the catalog lists. The backend checks the same (open_url in src-tauri/src/lib.rs).
const LINK_HOSTS = ["mise.jdx.dev", "github.com", "macos-defaults.com", "skills.sh", "docs.devin.ai"];

/** Homepages the catalog lists, which the backend also opens (lib.rs). */
const catalogLinks = new Set<string>();
export function trustCatalogLinks(urls: (string | null | undefined)[]) {
  for (const url of urls) if (url?.startsWith("https://")) catalogLinks.add(url);
}

/** Whether `url` is a link the app can open in the browser. */
export function canOpen(url: string | null | undefined): url is string {
  if (!url) return false;
  if (catalogLinks.has(url)) return true;
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" && LINK_HOSTS.includes(parsed.hostname);
  } catch {
    return false;
  }
}
