import { decodeBytes, encodeBytes } from "#ui/api/bytes.ts";
import { detectBranchRenames } from "#ui/api/ref-info.ts";
import type { RefInfo } from "@gitbutler/but-sdk";
import { expect, test } from "vitest";

const headInfo = (stacks: Array<Array<string | null>>): RefInfo =>
	({
		stacks: stacks.map((segments) => ({
			segments: segments.map((ref) => ({
				refName: ref === null ? null : { fullNameBytes: encodeBytes(ref) },
				commits: [],
			})),
		})),
	}) as unknown as RefInfo;

const renames = (prev: RefInfo, next: RefInfo) =>
	detectBranchRenames(prev, next).map(({ oldRef, newRef }) => [
		decodeBytes(oldRef),
		decodeBytes(newRef),
	]);

test("a branch replaced in place by a new name is a rename", () => {
	expect(
		renames(
			headInfo([["refs/heads/a", "refs/heads/old"], ["refs/heads/c"]]),
			headInfo([["refs/heads/a", "refs/heads/new"], ["refs/heads/c"]]),
		),
	).toEqual([["refs/heads/old", "refs/heads/new"]]);
});

test("an unchanged workspace has no renames", () => {
	const info = headInfo([["refs/heads/a"]]);
	expect(renames(info, headInfo([["refs/heads/a"]]))).toEqual([]);
});

test("a branch taking the place of one that moved is not a rename", () => {
	expect(
		renames(
			headInfo([["refs/heads/a"], ["refs/heads/b"]]),
			headInfo([["refs/heads/b"], ["refs/heads/a"]]),
		),
	).toEqual([]);
});

test("a removed branch is not a rename", () => {
	expect(
		renames(headInfo([["refs/heads/a"], ["refs/heads/b"]]), headInfo([["refs/heads/a"]])),
	).toEqual([]);
});

test("naming an anonymous segment is not a rename", () => {
	expect(renames(headInfo([[null]]), headInfo([["refs/heads/new"]]))).toEqual([]);
});
