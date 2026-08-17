import { mkdirSync, rmSync } from "node:fs";
import path from "node:path";

/**
 * Empties the capture directory once, before any test runs.
 *
 * This has to happen exactly once per run. Doing it in a `beforeAll` looks
 * equivalent and is not: Playwright restarts the worker after a failure, the
 * hook runs again, and the surfaces captured before the failure are deleted —
 * so a run with one broken surface reports the rest as missing rather than as
 * captured. It still has to happen at all, because a PNG left by an earlier run
 * would otherwise survive a rerun in which its own surface failed and be
 * compared as though this run had produced it.
 */
export default function clearScreenshots(): void {
	const out = process.env.SCREENSHOT_OUT;
	if (out === undefined || out === "") return;

	const outputDir = path.resolve(import.meta.dirname, "screenshots", out);
	rmSync(outputDir, { force: true, recursive: true });
	mkdirSync(outputDir, { recursive: true });
}
