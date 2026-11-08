import { getRangeExtractorWithIndices } from "./virtual.ts";
import { describe, expect, it } from "vitest";

// The default extractor's range: `overscan` rows either side of `startIndex`..`endIndex`.
const range = { startIndex: 10, endIndex: 12, overscan: 1, count: 100 };

describe("getRangeExtractorWithIndices", () => {
	it("returns the default range when nothing is pinned", () => {
		expect(getRangeExtractorWithIndices(range, [])).toEqual([9, 10, 11, 12, 13]);
	});

	it("leaves a pin already inside the default range alone", () => {
		expect(getRangeExtractorWithIndices(range, [11])).toEqual([9, 10, 11, 12, 13]);
	});

	it("orders a pin below the default range before it", () => {
		expect(getRangeExtractorWithIndices(range, [2])).toEqual([2, 9, 10, 11, 12, 13]);
	});

	it("orders a pin above the default range after it", () => {
		expect(getRangeExtractorWithIndices(range, [42])).toEqual([9, 10, 11, 12, 13, 42]);
	});

	it("orders pins on both sides", () => {
		expect(getRangeExtractorWithIndices(range, [42, 2])).toEqual([2, 9, 10, 11, 12, 13, 42]);
	});

	it("drops pins outside the item set", () => {
		expect(getRangeExtractorWithIndices(range, [-1, 100])).toEqual([9, 10, 11, 12, 13]);
	});
});
