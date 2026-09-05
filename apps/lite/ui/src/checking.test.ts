import { checkedRange, selectionAfterChecking } from "./checking.ts";
import { describe, expect, it } from "vitest";

describe("selectionAfterChecking", () => {
	for (const checked of [false, true]) {
		it.each([
			[null, null, null],
			[null, false, "next"],
			[null, true, null],
			[false, null, "previous"],
			[true, null, null],
			[true, true, null],
			[false, false, "next"],
			[true, false, "next"],
			[false, true, "previous"],
		] as const)(
			`after toggling to ${checked}, previous matches=%s, next matches=%s: %s`,
			(previous, next, expected) => {
				const states = {
					current: checked,
					previous: previous === null ? null : previous === checked,
					next: next === null ? null : next === checked,
				};
				expect(
					selectionAfterChecking({
						selection: "current" as keyof typeof states,
						getAdjacent: (offset) => (offset === 1 ? "next" : "previous"),
						getChecked: (item) => states[item],
					}),
				).toBe(expected);
			},
		);
	}

	it("stays on an uncheckable row", () => {
		expect(
			selectionAfterChecking({
				selection: "conflict",
				getAdjacent: () => "file",
				getChecked: (item) => (item === "conflict" ? null : false),
			}),
		).toBeNull();
	});

	it("fills then clears a run, reversing at its edge", () => {
		const items = ["a", "b", "c"];
		const checked = new Set<string>();
		let selection = "a";
		const positions = [];
		for (let step = 0; step < 6; step++) {
			if (checked.has(selection)) checked.delete(selection);
			else checked.add(selection);
			selection =
				selectionAfterChecking({
					selection,
					getAdjacent: (offset) => items[items.indexOf(selection) + offset] ?? null,
					getChecked: (item) => checked.has(item),
				}) ?? selection;
			positions.push(selection);
		}
		expect(positions).toEqual(["b", "c", "c", "b", "a", "a"]);
		expect(checked.size).toBe(0);
	});
});

describe("checkedRange", () => {
	const items = ["a", "b", "c", "d", "e"];

	const resolveRange = ({ anchor, target }: { anchor: string; target: string }) => {
		const anchorIndex = items.indexOf(anchor);
		const targetIndex = items.indexOf(target);
		if (anchorIndex === -1 || targetIndex === -1) return null;

		return new Set(
			items.slice(Math.min(anchorIndex, targetIndex), Math.max(anchorIndex, targetIndex) + 1),
		);
	};

	const getCheckedRange = checkedRange(resolveRange);

	it("checks a single item without shift", () => {
		expect(
			getCheckedRange({
				checked: new Set(),
				rangeAnchor: null,
				rangeEnd: null,
			})({ item: "c", shiftKey: false }),
		).toEqual({
			checked: new Set(["c"]),
			rangeAnchor: "c",
			rangeEnd: "c",
		});
	});

	it("treats shift as a normal click when nothing is checked", () => {
		expect(
			getCheckedRange({
				checked: new Set(),
				rangeAnchor: "c",
				rangeEnd: "e",
			})({ item: "a", shiftKey: true }),
		).toEqual({
			checked: new Set(["a"]),
			rangeAnchor: "a",
			rangeEnd: "a",
		});
	});

	it("checks an expanded range", () => {
		expect(
			getCheckedRange({
				checked: new Set(["a", "e"]),
				rangeAnchor: "a",
				rangeEnd: "a",
			})({ item: "c", shiftKey: true }),
		).toEqual({
			checked: new Set(["a", "b", "c", "e"]),
			rangeAnchor: "a",
			rangeEnd: "c",
		});
	});

	it("shrinks a checked range while retaining its anchor", () => {
		expect(
			getCheckedRange({
				checked: new Set(["a", "b", "c"]),
				rangeAnchor: "a",
				rangeEnd: "c",
			})({ item: "b", shiftKey: true }),
		).toEqual({
			checked: new Set(["a", "b"]),
			rangeAnchor: "a",
			rangeEnd: "b",
		});
	});

	it("unchecks items leaving a range even if they were previously checked", () => {
		let state = getCheckedRange({
			checked: new Set(["c"]),
			rangeAnchor: null,
			rangeEnd: null,
		})({ item: "a", shiftKey: false });

		state = getCheckedRange(state)({ item: "c", shiftKey: true });
		state = getCheckedRange(state)({ item: "b", shiftKey: true });

		expect(state).toEqual({
			checked: new Set(["a", "b"]),
			rangeAnchor: "a",
			rangeEnd: "b",
		});
	});

	it("checks the active range and unchecks the deactivated range", () => {
		expect(
			getCheckedRange({
				checked: new Set(["c", "d", "e"]),
				rangeAnchor: "c",
				rangeEnd: "e",
			})({ item: "a", shiftKey: true }),
		).toEqual({
			checked: new Set(["a", "b", "c"]),
			rangeAnchor: "c",
			rangeEnd: "a",
		});
	});

	it("unchecks the active range and checks the deactivated range", () => {
		expect(
			getCheckedRange({
				checked: new Set(["a", "b"]),
				rangeAnchor: "c",
				rangeEnd: "e",
			})({ item: "a", shiftKey: true }),
		).toEqual({
			checked: new Set(["d", "e"]),
			rangeAnchor: "c",
			rangeEnd: "a",
		});
	});

	it("falls back to a normal click when the range cannot be resolved", () => {
		expect(
			getCheckedRange({
				checked: new Set(["a"]),
				rangeAnchor: "missing",
				rangeEnd: "missing",
			})({ item: "c", shiftKey: true }),
		).toEqual({
			checked: new Set(["a", "c"]),
			rangeAnchor: "c",
			rangeEnd: "c",
		});
	});
});
