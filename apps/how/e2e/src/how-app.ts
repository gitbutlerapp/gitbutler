import { _electron as electron, type ElectronApplication, type Page } from "@playwright/test";
import path from "node:path";

export type HowApp = {
	app: ElectronApplication;
	page: Page;
};

export async function launchHowApp({
	projectPath,
	userDataPath,
}: {
	projectPath: string;
	userDataPath: string;
}): Promise<HowApp> {
	const env = {
		...process.env,
		HOW_E2E_PROJECT_PATH: projectPath,
		HOW_E2E_USER_DATA_DIR: userDataPath,
		HOW_CHECKPOINT_QUIET_MS: "100",
	};
	delete env.ELECTRON_RUN_AS_NODE;

	const app = await electron.launch({
		args: [path.resolve("dist/electron/main.js")],
		env,
	});
	const page = await app.firstWindow();
	await page.getByRole("heading", { name: "Manage changes." }).waitFor();
	return { app, page };
}
