// Shared machinery for the screenshot specs: where captures land, how a surface
// is photographed, and how one is reached at all.
//
// It lives apart from `tests/screenshots.spec.ts` so a throwaway spec can import
// it — see the `lite-screenshots` skill on capturing a surface the catalogue
// does not cover. Every workaround in here was earned; copying it into an ad-hoc
// file loses the comments explaining why.

import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { Page } from "@playwright/test";
import { expect } from "./test.ts";

const out = process.env.SCREENSHOT_OUT;
export const enabled = out !== undefined && out !== "";

// A wide-but-ordinary window, so panes are laid out the way a reviewer sees them.
export const VIEWPORT = { width: 1440, height: 900 };

export const outputDir = path.resolve(import.meta.dirname, "./screenshots", out ?? "unused");

export const shoot = async (appWindow: Page, name: string, selector: string): Promise<void> => {
	const target = appWindow.locator(selector);
	await expect(target).toBeVisible();
	// Let transitions and any late layout settle before the shutter.
	await appWindow.waitForTimeout(400);

	const box = await target.boundingBox();
	if (box === null) throw new Error(`${name}: ${selector} has no bounding box`);

	// Capture over the DevTools protocol rather than through page.screenshot().
	// Playwright waits for the compositor to hand it a fresh frame, and under a
	// bare X server this renderer only produces one on load, so that wait never
	// returns for a surface reached any other way. CDP takes what is on screen.
	const session = await appWindow.context().newCDPSession(appWindow);
	try {
		const shot = (await session.send("Page.captureScreenshot", {
			format: "png",
			clip: { x: box.x, y: box.y, width: box.width, height: box.height, scale: 1 },
		})) as { data: string };
		// Not only the catalogue spec writes here: an ad-hoc spec points
		// SCREENSHOT_OUT at a directory nothing has created yet.
		mkdirSync(outputDir, { recursive: true });
		writeFileSync(path.join(outputDir, `${name}.png`), Buffer.from(shot.data, "base64"));
	} finally {
		await session.detach();
	}
};

export const openProject = async (appWindow: Page): Promise<void> => {
	await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace/);
	await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();
	await appWindow.setViewportSize(VIEWPORT);

	// On CI the renderer the window starts with sometimes never produces a frame
	// ("sandboxed_renderer.bundle.js script failed to run"), and a screenshot then
	// waits for a paint that never arrives. Reloading gets a renderer that works.
	await appWindow.reload();
	await appWindow.getByRole("main").waitFor();
	await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();
};

// Under a bare X server the renderer only paints a capturable frame on load: a
// screenshot taken after an in-app state change waits for a frame that never
// arrives. So every surface is reached by navigating or reloading, never by
// clicking, and is photographed immediately afterwards — which is also why each
// surface gets its own test rather than sharing a window.
export const goToTab = async (
	appWindow: Page,
	tab: "workspace" | "upstream" | "branches",
): Promise<void> => {
	// The sidebar page lives in the query string, so this is a real navigation.
	await appWindow.evaluate((page) => {
		window.location.search = page === "workspace" ? "" : `?page=${page}`;
	}, tab);
	await appWindow.getByRole("main").waitFor();
	// Navigating alone leaves the renderer without a frame a screenshot can take;
	// only an explicit reload reliably produces one.
	await appWindow.reload();
	await appWindow.getByRole("main").waitFor();
	await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();

	// Assert the tab actually changed. Every other post-condition here is present
	// on all three tabs, so without this a navigation that silently stayed put
	// would write the workspace panel into branches-tab.png and pass. The
	// workspace is the absence of the parameter, so it is asserted the other way
	// round — `toContain("")` would pass on any tab at all.
	const search = async (): Promise<string> => new URL(appWindow.url()).search;
	if (tab === "workspace") await expect.poll(search).not.toContain("page=");
	else await expect.poll(search).toContain(`page=${tab}`);
};
