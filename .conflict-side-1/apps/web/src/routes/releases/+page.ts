// Re-export the downloads page data loader
// This allows /releases to load the same data as /downloads
//
// NOTE: This URL (https://gitbutler.com/releases) is referenced by the CLI
// after performing updates, so maintaining this route is important.
export { load, csr, ssr } from "../downloads/+page";
