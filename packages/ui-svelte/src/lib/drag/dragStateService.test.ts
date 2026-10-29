import { DragStateService } from "$lib/drag/dragStateService.svelte";
import { get } from "svelte/store";
import { describe, expect, it } from "vitest";

describe("DragStateService", () => {
	it("should handle rapid label changes without race conditions", () => {
		const service = new DragStateService();

		// Start dragging
		service.startDragging();

		// Simulate rapid movement between dropzones
		const token1 = service.setDropLabel("Zone A");
		expect(get(service.dropLabel)).toBe("Zone A");

		const token2 = service.setDropLabel("Zone B");
		expect(get(service.dropLabel)).toBe("Zone B");

		const token3 = service.setDropLabel("Zone C");
		expect(get(service.dropLabel)).toBe("Zone C");

		// Clear tokens in non-sequential order (simulating race condition)
		service.clearDropLabel(token1);
		// Label should still be 'Zone C' because token2 and token3 are still active
		expect(get(service.dropLabel)).toBe("Zone C");

		service.clearDropLabel(token3);
		// Label should now be 'Zone B' because it's the last remaining active label
		expect(get(service.dropLabel)).toBe("Zone B");

		service.clearDropLabel(token2);
		// Label should be undefined now
		expect(get(service.dropLabel)).toBeUndefined();
	});

	it("should track multiple concurrent labels correctly", () => {
		const service = new DragStateService();

		service.startDragging();

		// Set multiple labels
		const tokenA = service.setDropLabel("Label A");
		const tokenB = service.setDropLabel("Label B");
		const tokenC = service.setDropLabel("Label C");

		// Most recent label should be shown
		expect(get(service.dropLabel)).toBe("Label C");

		// Clear the most recent one
		service.clearDropLabel(tokenC);
		// Should fall back to the previous one
		expect(get(service.dropLabel)).toBe("Label B");

		// Clear in reverse order
		service.clearDropLabel(tokenA);
		// Should still show 'Label B'
		expect(get(service.dropLabel)).toBe("Label B");

		service.clearDropLabel(tokenB);
		expect(get(service.dropLabel)).toBeUndefined();
	});

	it("should clear all labels when dragging ends", () => {
		const service = new DragStateService();

		const cleanup = service.startDragging();

		const token1 = service.setDropLabel("Label 1");
		const token2 = service.setDropLabel("Label 2");

		expect(get(service.dropLabel)).toBe("Label 2");

		// End dragging should clear everything
		cleanup();

		expect(get(service.dropLabel)).toBeUndefined();
		expect(get(service.isDragging)).toBe(false);

		// Clearing tokens after drag ends should be safe (no-op)
		service.clearDropLabel(token1);
		service.clearDropLabel(token2);
		expect(get(service.dropLabel)).toBeUndefined();
	});

	it("should handle clearing non-existent tokens gracefully", () => {
		const service = new DragStateService();

		service.startDragging();

		const token1 = service.setDropLabel("Label 1");
		const fakeToken = Symbol("fake");

		// Clearing a fake token should not affect the current label
		service.clearDropLabel(fakeToken);
		expect(get(service.dropLabel)).toBe("Label 1");

		// Clearing the same token twice should be safe
		service.clearDropLabel(token1);
		expect(get(service.dropLabel)).toBeUndefined();

		service.clearDropLabel(token1);
		expect(get(service.dropLabel)).toBeUndefined();
	});
});
