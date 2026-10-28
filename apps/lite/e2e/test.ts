import { createRequire } from "node:module";
import {
	_electron,
	expect as baseExpect,
	test as base,
	type ElectronApplication,
	type Locator,
	type Page,
} from "@playwright/test";
import { App } from "./app.ts";
import { installForge, type FakeForge } from "./forge.ts";
import { Repo } from "./repo.ts";
import {
	createLiteTestEnvironment,
	devServerUrl,
	paths,
	processEnvironment,
	removeLiteTestEnvironment,
	seedScenario,
	type ObjectFormat,
} from "./setup.ts";

type TestOptions = {
	/** Installed before the test starts; null leaves the app without a forge. */
	forge: FakeForge | null;
	objectFormat: ObjectFormat;
	scenario: string | null;
};

type TestFixtures = {
	_electronArtifacts: void;
	app: App;
	appWindow: Page;
	electronApp: ElectronApplication;
	mainProcessLogs: Array<string>;
	repo: Repo;
	testEnvironment: ReturnType<typeof createLiteTestEnvironment>;
};

const require = createRequire(import.meta.url);
const electronPath = require("electron") as string;

export const test = base.extend<TestOptions & TestFixtures>({
	forge: [null, { option: true }],
	objectFormat: ["sha1", { option: true }],
	scenario: [null, { option: true }],
	// Playwright requires an object binding pattern even for fixtures without dependencies.
	// oxlint-disable-next-line no-empty-pattern
	mainProcessLogs: async ({}, provide) => {
		await provide([]);
	},
	testEnvironment: async ({ objectFormat, scenario }, provide) => {
		const environment = createLiteTestEnvironment();
		try {
			if (scenario !== null) await seedScenario(scenario, environment, objectFormat);
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
				...(devServerUrl !== undefined && { VITE_DEV_SERVER_URL: devServerUrl }),
			}),
		});
		app.on("console", (message) =>
			mainProcessLogs.push(`[main:${message.type()}] ${message.text()}`),
		);

		await provide(app);
		await app.close();
	},
	appWindow: async ({ electronApp, forge, viewport }, provide) => {
		const appWindow = await electronApp.firstWindow();
		if (viewport !== null) await appWindow.setViewportSize(viewport);
		const app = new App(appWindow, electronApp);
		await app.waitUntilReady();
		if (forge !== null) {
			await installForge(app, forge);
			await app.reload();
		}
		await provide(appWindow);
	},
	app: async ({ appWindow, electronApp }, provide) => {
		await provide(new App(appWindow, electronApp));
	},
	repo: async ({ app, testEnvironment }, provide) => {
		await provide(new Repo(testEnvironment, () => app.reload()));
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

const settles = (assertion: Promise<unknown>): Promise<boolean> =>
	assertion.then(
		() => true,
		() => false,
	);

export const expect = baseExpect.extend({
	/** The element shows `text` exactly, somewhere inside it. */
	async toShow(target: Pick<Locator, "getByText">, text: string | RegExp) {
		const element = target.getByText(text, { exact: true }).first();
		const pass = this.isNot
			? !(await settles(baseExpect(element).toBeHidden()))
			: await settles(baseExpect(element).toBeVisible());
		return {
			pass,
			name: "toShow",
			message: () => `expected ${this.isNot ? "not " : ""}to show ${String(text)}`,
		};
	},
	async toBeSelected(target: Locator) {
		const pass = this.isNot
			? !(await settles(baseExpect(target).not.toHaveAttribute("aria-selected", "true")))
			: await settles(baseExpect(target).toHaveAttribute("aria-selected", "true"));
		return {
			pass,
			name: "toBeSelected",
			message: () => `expected ${String(target)} ${this.isNot ? "not " : ""}to be selected`,
		};
	},
	async toHaveHeight(target: Locator, height: number) {
		let last: number | undefined;
		const pass = await settles(
			baseExpect.poll(async () => (last = (await target.boundingBox())?.height)).toBe(height),
		);
		return {
			pass,
			name: "toHaveHeight",
			message: () => `expected ${String(target)} to be ${height}px tall, it is ${String(last)}px`,
		};
	},
	/** The array begins with `prefix`, in order. */
	toStartWith(received: Array<unknown>, prefix: Array<unknown>) {
		const start = received.slice(0, prefix.length);
		const pass = JSON.stringify(start) === JSON.stringify(prefix);
		return {
			pass,
			name: "toStartWith",
			message: () =>
				`expected ${JSON.stringify(received, null, 2)}\n` +
				`${pass ? "not " : ""}to start with ${JSON.stringify(prefix, null, 2)}`,
		};
	},
	/** Hovering the element changes nothing about its box, once `revealing` shows. */
	async toHoldStillOnHover(target: Locator, { revealing }: { revealing: Locator }) {
		await target.page().mouse.move(0, 0);
		const before = await target.boundingBox();
		await target.hover();
		await baseExpect(revealing).toBeVisible();
		const after = await target.boundingBox();
		const pass = JSON.stringify(after) === JSON.stringify(before);
		return {
			pass,
			name: "toHoldStillOnHover",
			message: () =>
				`expected ${String(target)} ${pass ? "to move" : "to hold still"} on hover\n` +
				`before: ${JSON.stringify(before)}\nafter:  ${JSON.stringify(after)}`,
		};
	},
	/** Focus settles inside the `scope` focus scope. */
	async toHaveFocusIn(app: App, scope: string) {
		let last: string | null = null;
		const pass = await baseExpect
			.poll(async () => (last = await app.focusedScope()))
			.toBe(scope)
			.then(
				() => true,
				() => false,
			);
		return {
			pass,
			name: "toHaveFocusIn",
			message: () => `expected focus in "${scope}", found it in "${last ?? "nothing"}"`,
		};
	},
	/** None of the element's text is cut off at its bottom edge. */
	async toShowItsTextUnclipped(target: Locator) {
		const { text, clip } = await target.evaluate((element) => {
			const range = document.createRange();
			range.selectNodeContents(element);
			return {
				text: range.getBoundingClientRect().bottom,
				clip: element.getBoundingClientRect().bottom,
			};
		});
		const pass = text <= clip;
		return {
			pass,
			name: "toShowItsTextUnclipped",
			message: () =>
				`expected ${String(target)}'s text ${pass ? "to be" : "not to be"} clipped: ` +
				`text ends at ${text}, the element at ${clip}`,
		};
	},
});
