import { expect, test } from "../test.ts";

type PopupPosition = { x?: number; y?: number };

test("a native menu opens at the requested point when the page is zoomed", async ({ app }) => {
	await app.electronApp.evaluate(({ BrowserWindow, Menu }) => {
		const state = globalThis as { popups?: Array<PopupPosition> };
		state.popups = [];
		Menu.prototype.popup = (options) => {
			state.popups?.push({ x: options?.x, y: options?.y });
			options?.callback?.();
		};
		BrowserWindow.getAllWindows()[0]?.webContents.setZoomFactor(1.5);
	});

	await app.page.evaluate(() =>
		window.lite.showNativeMenu({
			items: [{ _tag: "Item", label: "Probe", itemId: "probe" }],
			position: { x: 100, y: 40 },
		}),
	);

	// The renderer sends CSS pixels; the popup is placed in window points.
	const popups = await app.electronApp.evaluate(
		() => (globalThis as { popups?: Array<PopupPosition> }).popups,
	);
	expect(popups).toEqual([{ x: 150, y: 60 }]);
});
