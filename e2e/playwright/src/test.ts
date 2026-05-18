import { serverLogSink } from "./serverLog.ts";
import { startGitButler, type GitButler } from "./setup.ts";
import { test as base } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

export type GitButlerOptions = {
	env?: Record<string, string>;
	config?: Record<string, unknown>;
};

const FLAT_RESULTS_DIR = path.resolve(import.meta.dirname, "../test-results-flat");

/**
 * Sanitize a string for use as a filename.
 */
function sanitizeFilename(name: string): string {
	return name
		.replace(/[^a-zA-Z0-9_-]/g, "-")
		.replace(/-+/g, "-")
		.replace(/^-|-$/g, "");
}

/**
 * Extended test fixture that, on failure, writes console logs, video,
 * trace, and but-server logs to a flat directory.
 *
 * Output: e2e/playwright/test-results-flat/
 *   <suite>--<test-name>-client.txt   (browser console)
 *   <suite>--<test-name>-server.txt   (but-server stdout/stderr)
 *   <suite>--<test-name>.webm         (video)
 *   <suite>--<test-name>-trace.zip    (trace)
 */
export const test = base.extend<{
	_autoArtifacts: void;
	gitbutlerOptions: GitButlerOptions;
	gitbutler: GitButler;
}>({
	gitbutlerOptions: [{ config: { onboardingComplete: true } }, { option: true }],
	gitbutler: async ({ context, gitbutlerOptions }, use, testInfo) => {
		const workdir = testInfo.outputPath("workdir");
		const configdir = testInfo.outputPath("config");
		const instance = await startGitButler(
			workdir,
			configdir,
			context,
			gitbutlerOptions.env,
			gitbutlerOptions.config,
		);
		await use(instance);
		await instance.destroy();
	},
	_autoArtifacts: [
		async ({ page }, use, testInfo) => {
			const logs: string[] = [];
			serverLogSink.length = 0;

			page.on("console", (msg) => {
				const type = msg.type().toUpperCase().padEnd(7);
				logs.push(`[${type}] ${msg.text()}`);
			});

			page.on("pageerror", (err) => {
				logs.push(`[ERROR  ] ${err.message}`);
			});

			await use();

			if (testInfo.status !== testInfo.expectedStatus) {
				const titlePath = testInfo.titlePath.slice(1);
				const baseName = sanitizeFilename(titlePath.join("--"));
				const retry = testInfo.retry > 0 ? `-retry${testInfo.retry}` : "";
				const prefix = `${baseName}${retry}`;

				fs.mkdirSync(FLAT_RESULTS_DIR, { recursive: true });

				if (logs.length > 0) {
					fs.writeFileSync(path.join(FLAT_RESULTS_DIR, `${prefix}-console.txt`), logs.join("\n"));
				}

				if (serverLogSink.length > 0) {
					fs.writeFileSync(
						path.join(FLAT_RESULTS_DIR, `${prefix}-server.txt`),
						serverLogSink.join("\n"),
					);
				}

				// Close the browser context to finalize video and trace recordings.
				await page.context().close();
				const video = page.video();
				if (video) {
					await video.saveAs(path.join(FLAT_RESULTS_DIR, `${prefix}.webm`));
				}

				// Copy trace if it was retained
				const traceSource = path.join(testInfo.outputDir, "trace.zip");
				if (fs.existsSync(traceSource)) {
					fs.copyFileSync(traceSource, path.join(FLAT_RESULTS_DIR, `${prefix}-trace.zip`));
				}
			}
		},
		{ auto: true },
	],
});
