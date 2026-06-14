import { _electron as electron, type ElectronApplication, type Page } from "@playwright/test";
import path from "node:path";

export type HowApp = {
	app: ElectronApplication;
	page: Page;
};

function e2eSlowMoMs(): number | undefined {
	const raw = process.env.HOW_E2E_SLOW_MO_MS;
	if (!raw) return undefined;
	const parsed = Number(raw);
	return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
}

export async function launchHowApp({
	projectPath,
	userDataPath,
	checkpointQuietMs = "100",
	checkpointSummary,
	checkpointSummaryDelayMs,
	checkpointSummaryError,
	checkpointSummaryTimeoutMs,
}: {
	projectPath: string;
	userDataPath: string;
	checkpointQuietMs?: string;
	checkpointSummary?: string;
	checkpointSummaryDelayMs?: string;
	checkpointSummaryError?: string;
	checkpointSummaryTimeoutMs?: string;
}): Promise<HowApp> {
	const env = {
		...process.env,
		HOW_E2E_PROJECT_PATH: projectPath,
		HOW_E2E_USER_DATA_DIR: userDataPath,
		HOW_CHECKPOINT_QUIET_MS: checkpointQuietMs,
	};
	if (checkpointSummary !== undefined) env.HOW_E2E_CHECKPOINT_SUMMARY = checkpointSummary;
	if (checkpointSummaryDelayMs !== undefined)
		env.HOW_E2E_CHECKPOINT_SUMMARY_DELAY_MS = checkpointSummaryDelayMs;
	if (checkpointSummaryError !== undefined)
		env.HOW_E2E_CHECKPOINT_SUMMARY_ERROR = checkpointSummaryError;
	if (checkpointSummaryTimeoutMs !== undefined)
		env.HOW_CHECKPOINT_SUMMARY_TIMEOUT_MS = checkpointSummaryTimeoutMs;
	delete env.ELECTRON_RUN_AS_NODE;

	const launchOptions: Parameters<typeof electron.launch>[0] = {
		args: [path.resolve("dist/electron/main.js")],
		env,
	};
	const slowMo = e2eSlowMoMs();
	if (slowMo) launchOptions.slowMo = slowMo;

	const app = await electron.launch(launchOptions);
	const page = await app.firstWindow();
	await page.waitForLoadState("domcontentloaded");
	await page.locator("body").waitFor();
	return { app, page };
}
