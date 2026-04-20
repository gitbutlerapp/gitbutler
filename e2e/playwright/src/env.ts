import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "../../..");
const E2E_DIR = path.resolve(ROOT, "e2e/playwright");

/** Append `.exe` on Windows when building a default binary path. */
function binaryPath(...segments: string[]): string {
	const p = path.join(...segments);
	return process.platform === "win32" ? p + ".exe" : p;
}

export const BUT_SERVER_PORT = process.env.BUTLER_PORT || "6978";
export const DESKTOP_PORT = process.env.DESKTOP_PORT || "3000";
export const BUT_TESTING =
	process.env.BUT_TESTING || binaryPath(ROOT, "target", "debug", "but-testing");
export const BUT_SERVER =
	process.env.BUT_SERVER || binaryPath(ROOT, "target", "debug", "but-server");
export const GIT_CONFIG_GLOBAL =
	process.env.GIT_CONFIG_GLOBAL || path.join(E2E_DIR, "fixtures", ".gitconfig");
