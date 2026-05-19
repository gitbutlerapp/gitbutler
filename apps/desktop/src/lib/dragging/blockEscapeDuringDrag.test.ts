import { blockEscapeDuringDrag } from "$lib/dragging/blockEscapeDuringDrag";
import { describe, expect, test } from "vitest";

describe("blockEscapeDuringDrag", () => {
	test("prevents Escape while a drag is active", () => {
		const release = blockEscapeDuringDrag();
		const event = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });

		window.dispatchEvent(event);
		release();

		expect(event.defaultPrevented).toBe(true);
	});

	test("allows Escape again after cleanup", () => {
		const release = blockEscapeDuringDrag();
		release();

		const event = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
		window.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(false);
	});

	test("keeps Escape blocked until the last drag ends", () => {
		const releaseFirst = blockEscapeDuringDrag();
		const releaseSecond = blockEscapeDuringDrag();

		releaseFirst();

		const duringEvent = new KeyboardEvent("keydown", {
			key: "Escape",
			bubbles: true,
			cancelable: true,
		});
		window.dispatchEvent(duringEvent);

		releaseSecond();

		const afterEvent = new KeyboardEvent("keydown", {
			key: "Escape",
			bubbles: true,
			cancelable: true,
		});
		window.dispatchEvent(afterEvent);

		expect(duringEvent.defaultPrevented).toBe(true);
		expect(afterEvent.defaultPrevented).toBe(false);
	});
});
