/** @vitest-environment jsdom */

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RelativeTime } from "./RelativeTime.tsx";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe("RelativeTime", () => {
	let container: HTMLDivElement;
	let root: Root;
	/** Pinned so the test reads the wording, not the machine's clock. */
	const start = Date.parse("2026-08-29T12:00:00Z");

	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(start);
		container = document.createElement("div");
		document.body.append(container);
		root = createRoot(container);
	});

	afterEach(() => {
		act(() => root.unmount());
		container.remove();
		vi.useRealTimers();
	});

	it("ages in place while nothing else re-renders it", () => {
		act(() => root.render(<RelativeTime timestamp={start} />));
		expect(container.textContent).toBe("just now");

		act(() => {
			vi.advanceTimersByTime(5 * 60_000);
		});

		expect(container.textContent).toBe("5 minutes ago");
	});

	it("holds still when the caller pins the clock", () => {
		act(() => root.render(<RelativeTime timestamp={start} now={start} />));

		act(() => {
			vi.advanceTimersByTime(5 * 60_000);
		});

		// A pinned list stays stable no matter how long it is left open.
		expect(container.textContent).toBe("just now");
	});
});
