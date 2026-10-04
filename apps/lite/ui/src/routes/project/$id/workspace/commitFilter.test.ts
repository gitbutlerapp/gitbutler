import { describe, expect, it } from "vitest";
import {
	filterSpan,
	toggleCommit,
	unpushedCount,
} from "#ui/routes/project/$id/workspace/commitFilter.ts";

const local = (id: string) => ({ id, state: { type: "LocalOnly" } });
const pushed = (id: string) => ({ id, state: { type: "LocalAndRemote" } });

// Newest first, as a segment lists them: d and c not pushed yet.
const commits = [local("d"), local("c"), pushed("b"), pushed("a")];

const range = (newest: string, oldest: string) => ({ _tag: "Range", newest, oldest }) as const;
const all = { _tag: "All" } as const;

describe("toggleCommit", () => {
	it("ticking a commit from all shows that commit alone", () => {
		expect(toggleCommit(all, commits, "b")).toEqual(range("b", "b"));
	});

	it("ticking outside the run stretches it to reach the commit", () => {
		expect(toggleCommit(range("c", "c"), commits, "a")).toEqual(range("c", "a"));
		expect(toggleCommit(range("b", "b"), commits, "d")).toEqual(range("d", "b"));
	});

	it("ticking every commit is the whole branch again", () => {
		expect(toggleCommit(range("d", "b"), commits, "a")).toEqual(all);
	});

	it("unticking an end shrinks the run by one", () => {
		expect(toggleCommit(range("d", "b"), commits, "b")).toEqual(range("d", "c"));
		expect(toggleCommit(range("d", "b"), commits, "d")).toEqual(range("c", "b"));
	});

	it("unticking inside the run drops that commit and the older ones", () => {
		expect(toggleCommit(range("d", "b"), commits, "c")).toEqual(range("d", "d"));
	});

	it("unticking the only commit is the whole branch again", () => {
		expect(toggleCommit(range("c", "c"), commits, "c")).toEqual(all);
	});

	it("a preset gives way to the ticked commit", () => {
		expect(toggleCommit({ _tag: "Unpushed" }, commits, "a")).toEqual(range("a", "a"));
	});
});

describe("filterSpan", () => {
	it("covers the unpushed run at the tip", () => {
		expect(unpushedCount(commits)).toBe(2);
		expect(filterSpan({ _tag: "Unpushed" }, commits)).toEqual([0, 1]);
	});

	it("is the whole branch when every commit is unpushed", () => {
		expect(filterSpan({ _tag: "Unpushed" }, [local("b"), local("a")])).toBeNull();
	});

	it("falls back to the whole branch when a named commit is gone", () => {
		expect(filterSpan(range("x", "b"), commits)).toBeNull();
	});
});
