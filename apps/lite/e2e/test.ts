import { createRequire } from "node:module";
import {
	_electron,
	expect,
	test as base,
	type ElectronApplication,
	type Page,
} from "@playwright/test";
import {
	createLiteTestEnvironment,
	paths,
	processEnvironment,
	removeLiteTestEnvironment,
	seedScenario,
} from "./setup.ts";

type TestOptions = {
	scenario: string | null;
};

type TestFixtures = {
	_electronArtifacts: void;
	appWindow: Page;
	electronApp: ElectronApplication;
	mainProcessLogs: Array<string>;
	testEnvironment: ReturnType<typeof createLiteTestEnvironment>;
};

const require = createRequire(import.meta.url);
const electronPath = require("electron") as string;

export const test = base.extend<TestOptions & TestFixtures>({
	scenario: [null, { option: true }],
	// Playwright requires an object binding pattern even for fixtures without dependencies.
	// oxlint-disable-next-line no-empty-pattern
	mainProcessLogs: async ({}, provide) => {
		await provide([]);
	},
	testEnvironment: async ({ scenario }, provide) => {
		const environment = createLiteTestEnvironment();
		try {
			if (scenario !== null) await seedScenario(scenario, environment);
			await provide(environment);
		} finally {
			removeLiteTestEnvironment(environment);
		}
	},
	electronApp: async ({ headless, mainProcessLogs, testEnvironment }, provide) => {
		const app = await _electron.launch({
			executablePath: electronPath,
			args: [`--user-data-dir=${testEnvironment.electronUserDataDir}`, paths.electronMain],
			env: processEnvironment({
				E2E_TEST_APP_DATA_DIR: testEnvironment.appDataDir,
				GIT_CONFIG_GLOBAL: testEnvironment.gitConfig,
				GITBUTLER_LITE_HEADLESS: String(headless),
				VITE_DEV_SERVER_URL: "http://127.0.0.1:5173",
			}),
		});
		app.on("console", (message) =>
			mainProcessLogs.push(`[main:${message.type()}] ${message.text()}`),
		);

		await provide(app);
		await app.close();
	},
	appWindow: async ({ electronApp }, provide) => {
		const appWindow = await electronApp.firstWindow();
		await appWindow.setViewportSize({ width: 1024, height: 768 });
		await appWindow.getByRole("main").waitFor();
		await provide(appWindow);
	},
	_electronArtifacts: [
		async ({ appWindow, mainProcessLogs }, provide, testInfo) => {
			await provide();
			if (testInfo.status === testInfo.expectedStatus) return;

			const [consoleMessages, pageErrors] = await Promise.all([
				appWindow.consoleMessages(),
				appWindow.pageErrors(),
			]);
			const logs = [
				...mainProcessLogs,
				...consoleMessages.map((message) => `[renderer:${message.type()}] ${message.text()}`),
				...pageErrors.map((error) => `[renderer:error] ${error.stack ?? error.message}`),
			];

			if (logs.length === 0) return;
			await testInfo.attach("electron.log", {
				body: Buffer.from(logs.join("\n")),
				contentType: "text/plain",
			});
		},
		{ auto: true },
	],
});

export { expect };
