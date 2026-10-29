import { fileLanguages } from "$lib/analytics/fileLanguages";
import { describe, expect, test } from "vitest";
import type { DiffSpec } from "@gitbutler/but-sdk";

function changes(...paths: string[]): DiffSpec[] {
	const encoder = new TextEncoder();
	return paths.map((path) => ({
		pathBytes: Array.from(encoder.encode(path)),
		previousPathBytes: null,
		hunkHeaders: [],
	}));
}

describe("fileLanguages", () => {
	test("maps known extensions, keeps unknown ones, and dedupes", () => {
		const result = fileLanguages(
			changes("src/a.ts", "src/b.MTS", "lib.rs", "x/Main.rs", "app.zig"),
		);
		expect(result).toEqual(["rust", "typescript", "zig"]);
	});

	test("never reports a file name", () => {
		const result = fileLanguages(
			changes("Makefile", "LICENSE", ".env", "config/.npmrc", "acme-corp.backup-2024", "lib.so.1"),
		);
		expect(result).toEqual([]);
	});
});
