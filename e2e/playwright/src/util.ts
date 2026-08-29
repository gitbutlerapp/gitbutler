import { IDLE_BUDGET_MS, idleSince, watchIdle } from "./idle.ts";
import { TestId } from "@gitbutler/ui/utils/testIds";
import { type Locator, type Page } from "@playwright/test";

type TestIdValues = `${TestId}`;

/**
 * Platform modifier key for multi-select (Cmd on macOS, Ctrl elsewhere).
 */
export const MOD_KEY: "Meta" | "Control" = process.platform === "darwin" ? "Meta" : "Control";

/**
 * Get by test ID from the page.
 *
 * This is only here in order to have nice autocompletion in the IDE.
 */
export function getByTestId(page: Page, testId: TestIdValues) {
	return page.getByTestId(testId);
}

/**
 * Locator for a commit row, optionally filtered by its visible text.
 */
export function commitRow(page: Page, hasText?: string): Locator {
	const base = page.getByTestId("commit-row");
	return hasText ? base.filter({ hasText }) : base;
}

/**
 * Locator for a stack. When `branchName` is provided, finds the stack
 * that contains a branch header matching that name. This is more reliable
 * than `.filter({ hasText })` because commit messages can also mention
 * branch names.
 */
export function stack(page: Page, branchName?: string): Locator {
	const base = page.getByTestId("stack");
	if (!branchName) return base;
	return base.filter({
		has: page.getByTestId("branch-header").filter({ hasText: branchName }),
	});
}

/** How often the watchdog samples; small enough to be precise, large enough to be free. */
const IDLE_POLL_MS = 250;

/**
 * Run `act` with no deadline of its own, failing once the app has been idle for
 * `idleBudgetMs` without it finishing. See `idle.ts` for why idleness rather
 * than a wall clock decides.
 */
async function untilTheAppGoesQuiet<T>(
	page: Page,
	describe: string,
	idleBudgetMs: number,
	act: () => Promise<T>,
): Promise<T> {
	watchIdle(page);
	const startedAt = Date.now();
	let done = false;

	const acting = act().finally(() => {
		done = true;
	});
	// The watchdog may win the race; keep its loser from surfacing as an
	// unhandled rejection.
	acting.catch(() => {});

	const watchdog = (async (): Promise<T> => {
		for (;;) {
			await new Promise((resolve) => setTimeout(resolve, IDLE_POLL_MS));
			// Stop counting the moment the action settles, so the loop cannot
			// outlive it — `acting` has already resolved or rejected here.
			if (done) return await acting;
			const idleFor = idleSince(page, startedAt);
			if (idleFor >= idleBudgetMs) {
				throw new Error(
					`Timed out ${describe}: the app made no request for ` +
						`${Math.round(idleFor / 1000)}s, so it is not still working on it.`,
				);
			}
		}
	})();

	return await Promise.race([acting, watchdog]);
}

/**
 * Hover, bounded by app silence rather than by `actionTimeout`.
 *
 * Playwright waits for an element to be stable before hovering it, and a list
 * still reflowing as its data arrives is not stable yet. That is the app being
 * slow, not the action being stuck, so it gets the same rule as the waits.
 */
export async function hoverPatiently(
	page: Page,
	locator: Locator,
	options?: { force?: boolean; position?: { x: number; y: number }; idleBudgetMs?: number },
): Promise<void> {
	await untilTheAppGoesQuiet(
		page,
		"hovering",
		options?.idleBudgetMs ?? IDLE_BUDGET_MS,
		async () =>
			await locator.hover({
				force: options?.force,
				position: options?.position,
				timeout: 0,
			}),
	);
}

export async function waitForTestId(
	page: Page,
	testId: TestIdValues,
	options?: { idleBudgetMs?: number },
): Promise<Locator> {
	const element = getByTestId(page, testId);
	await untilTheAppGoesQuiet(
		page,
		`waiting for getByTestId(${JSON.stringify(testId)}) to be visible`,
		options?.idleBudgetMs ?? IDLE_BUDGET_MS,
		async () => await element.waitFor({ state: "visible", timeout: 0 }),
	);
	return element;
}

export async function waitForTestIdToNotExist(
	page: Page,
	testId: TestIdValues,
	options?: { idleBudgetMs?: number },
): Promise<void> {
	await untilTheAppGoesQuiet(
		page,
		`waiting for getByTestId(${JSON.stringify(testId)}) to be detached`,
		options?.idleBudgetMs ?? IDLE_BUDGET_MS,
		async () => await getByTestId(page, testId).waitFor({ state: "detached", timeout: 0 }),
	);
}

/**
 * Click an element by test ID.
 */
export async function clickByTestId(
	page: Page,
	testId: TestIdValues,
	force?: boolean,
): Promise<Locator> {
	const element = await waitForTestId(page, testId);
	await element.click({
		force,
	});
	return element;
}

export async function rightClickByTestId(page: Page, testId: TestIdValues): Promise<Locator> {
	const element = await waitForTestId(page, testId);
	await element.click({
		button: "right",
	});
	return element;
}

/**
 * Drag and drop an element onto another element by their test IDs.
 */
export async function dragAndDropByTestId(
	page: Page,
	sourceId: TestIdValues,
	targetId: TestIdValues,
) {
	const source = await waitForTestId(page, sourceId);
	const target = await waitForTestId(page, targetId);

	await hoverPatiently(page, source);
	await page.mouse.down();
	await hoverPatiently(page, target);
	await hoverPatiently(page, target, { force: true });
	await page.mouse.up();
}

type DropOptions = {
	force?: boolean;
	position?: {
		x: number;
		y: number;
	};
};

/**
 * Drag and drop an element onto another element by their locators.
 */
export async function dragAndDropByLocator(
	page: Page,
	source: Locator,
	target: Locator,
	options: DropOptions = {},
) {
	await hoverPatiently(page, source);
	await page.mouse.down();
	// Always wait a bit in case CSS causes content shift.
	await page.waitForTimeout(100);
	await hoverPatiently(page, target, { force: options.force, position: options.position });
	// The drag system uses requestAnimationFrame to detect dropzones via
	// document.elementFromPoint. Wait for at least one animation frame so the
	// dropzone is detected as hovered before we release the mouse button.
	// eslint-disable-next-line @typescript-eslint/promise-function-async
	await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));
	await page.mouse.up();
}

export async function fillByTestId(
	page: Page,
	testId: TestIdValues,
	value: string,
): Promise<Locator> {
	const element = await waitForTestId(page, testId);
	// Fill can race a re-render that resets the field, so retry until the value
	// sticks — under the idle budget like every other wait. A function-subject
	// `toPass()` would bypass src/expect.ts and burn the per-test timeout on a
	// real failure.
	await untilTheAppGoesQuiet(
		page,
		`filling getByTestId(${JSON.stringify(testId)})`,
		IDLE_BUDGET_MS,
		async () => {
			// timeout: 0 on the inner calls too — the watchdog is the boundary
			// here, and a 15s actionTimeout inside the loop would reintroduce a
			// wall-clock failure while the app is still visibly working.
			for (;;) {
				await element.fill(value, { timeout: 0 });
				if ((await element.inputValue({ timeout: 0 })) === value) return;
				await new Promise((resolve) => setTimeout(resolve, IDLE_POLL_MS));
			}
		},
	);
	return element;
}

/**
 * Type into the rich text editor by test ID.
 *
 * Only use this for the rich text editor, as this is a workaround for the fact that
 * the rich text editor does not support the `fill` method.
 *
 * If you need to pass text into a norma input element, @see fillByTestId instead
 */
export async function textEditorFillByTestId(page: Page, testId: TestIdValues, value: string) {
	const element = await waitForTestId(page, testId);
	await element.click();
	await element.pressSequentially(value);
	return element;
}

/**
 * Wait until an element's bounding box stops changing between animation frames.
 * Useful for popups positioned by Floating UI that take a few frames to settle.
 */
export async function waitForElementToStabilize(page: Page, locator: Locator, timeout = 10000) {
	const start = Date.now();
	let lastBox = await locator.boundingBox();
	while (Date.now() - start < timeout) {
		// eslint-disable-next-line @typescript-eslint/promise-function-async
		await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));
		const box = await locator.boundingBox();
		if (
			box &&
			lastBox &&
			Math.abs(box.x - lastBox.x) < 1 &&
			Math.abs(box.y - lastBox.y) < 1 &&
			Math.abs(box.width - lastBox.width) < 1 &&
			Math.abs(box.height - lastBox.height) < 1
		) {
			return;
		}
		lastBox = box;
	}
	throw new Error(
		`Element did not stabilize within ${timeout}ms — last bounding box: ${JSON.stringify(lastBox)}`,
	);
}

/**
 * Mock the backend's native directory picker to return a specific path.
 *
 * The web frontend calls `POST /pick_directory` to open a native OS file dialog.
 * In e2e tests we intercept this request and return the desired path directly.
 * Must be called before the action that triggers the picker.
 */
export async function mockPickDirectory(page: Page, directoryPath: string): Promise<void> {
	await page.unroute("**/pick_directory");
	await page.route("**/pick_directory", async (route) => {
		await route.fulfill({
			status: 200,
			contentType: "application/json",
			body: JSON.stringify({ type: "success", subject: { path: directoryPath } }),
		});
	});
}
