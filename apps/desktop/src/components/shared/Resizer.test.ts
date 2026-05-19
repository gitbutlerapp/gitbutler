import Resizer from "$components/shared/Resizer.svelte";
import { RESIZE_SYNC, ResizeSync } from "$lib/floating/resizeSync";
import { SASH_LAYER, type SashLayerContext } from "$lib/sash/sashLayer";
import { UI_STATE } from "$lib/state/uiState.svelte";
import { fireEvent, render, waitFor } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";

function mockUiState() {
	return {
		global: {
			zoom: { current: 1, set: vi.fn() },
		},
	};
}

function baseContext(): Map<any, any> {
	return new Map<any, any>([
		[UI_STATE._key, mockUiState()],
		[RESIZE_SYNC._key, new ResizeSync()],
	]);
}

function baseProps() {
	const viewport = document.createElement("div");
	viewport.style.width = "300px";
	viewport.style.height = "200px";
	return {
		defaultValue: 20,
		viewport,
		direction: "right" as const,
	};
}

describe("Resizer", () => {
	test("throws when mounted without SashLayer context", () => {
		expect(() => {
			render(Resizer, {
				props: baseProps(),
				context: baseContext(),
			});
		}).toThrow("Resizer must be used inside <SashLayer>.");
	});

	test("mounts when SashLayer context is provided", async () => {
		const container = document.createElement("div");
		document.body.appendChild(container);
		const layerCtx: SashLayerContext = {
			container,
			requestLayout: vi.fn(),
			subscribeLayout: () => () => {},
			observeLayoutTarget: () => () => {},
			setAutoLayoutPaused: vi.fn(),
		};

		const context = baseContext();
		context.set(SASH_LAYER, layerCtx);

		const { unmount } = render(Resizer, {
			props: baseProps(),
			context,
		});

		await waitFor(() => expect(container.querySelector(".resizer")).toBeInTheDocument());
		unmount();
		container.remove();
	});

	test("persists resizes only on pointer up", async () => {
		const container = document.createElement("div");
		document.body.appendChild(container);
		const layerCtx: SashLayerContext = {
			container,
			requestLayout: vi.fn(),
			subscribeLayout: () => () => {},
			observeLayoutTarget: () => () => {},
			setAutoLayoutPaused: vi.fn(),
		};
		const context = baseContext();
		context.set(SASH_LAYER, layerCtx);
		const props = baseProps();
		Object.defineProperty(props.viewport, "clientWidth", {
			configurable: true,
			value: 300,
		});
		const setItem = vi.spyOn(Storage.prototype, "setItem");

		const { unmount } = render(Resizer, {
			props: {
				...props,
				persistId: "resizer-test-width",
				minWidth: 10,
				maxWidth: 50,
			},
			context,
		});

		const resizer = await waitFor(() => {
			const element = container.querySelector(".resizer");
			expect(element).toBeInTheDocument();
			return element as HTMLElement;
		});
		setItem.mockClear();

		await fireEvent(
			resizer,
			new MouseEvent("pointerdown", { clientX: 300, clientY: 0, bubbles: true }),
		);
		await fireEvent(
			document,
			new MouseEvent("pointermove", { clientX: 420, clientY: 0, bubbles: true }),
		);
		await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

		expect(props.viewport.style.width).toBe("26.25rem");
		expect(setItem).not.toHaveBeenCalled();

		await fireEvent(document, new MouseEvent("pointerup", { bubbles: true }));

		expect(setItem).toHaveBeenCalled();

		unmount();
		container.remove();
		setItem.mockRestore();
	});
});
