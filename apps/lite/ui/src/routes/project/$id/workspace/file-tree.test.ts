import { describe, expect, test } from "vitest";
import {
	buildFileTreeRows,
	parentDirectoryRow,
	selectedFilePath,
	type FileTreeRow,
} from "./file-tree.ts";

const items = (...paths: Array<string>) => paths.map((path) => ({ path }));

const layout = (rows: Array<FileTreeRow<{ path: string }>>): Array<string> =>
	rows.map(
		(row) => `${"  ".repeat(row.depth)}${row._tag === "Directory" ? `${row.name}/` : row.path}`,
	);

const tree = (paths: Array<string>, collapsedDirectories: Record<string, true> = {}) =>
	buildFileTreeRows({ items: items(...paths), mode: "tree", collapsedDirectories });

describe("buildFileTreeRows", () => {
	test("list mode follows tree order, with whole paths at one level", () => {
		const rows = buildFileTreeRows({
			items: items("src/b.ts", "a.ts", "src/lib/c.ts"),
			mode: "list",
			collapsedDirectories: { src: true },
		});

		expect(layout(rows)).toEqual(["src/lib/c.ts", "src/b.ts", "a.ts"]);
	});

	test("groups files under their directories, directories first", () => {
		const rows = tree(["readme.md", "src/app.ts", "src/ui/row.ts", "docs/guide.md"]);

		expect(layout(rows)).toEqual([
			"docs/",
			"  docs/guide.md",
			"src/",
			"  ui/",
			"    src/ui/row.ts",
			"  src/app.ts",
			"readme.md",
		]);
	});

	test("folds a chain of sole-child directories into one row", () => {
		const rows = tree(["src/lib/files/row.ts", "src/lib/files/tree.ts"]);

		expect(layout(rows)).toEqual([
			"src/lib/files/",
			"  src/lib/files/row.ts",
			"  src/lib/files/tree.ts",
		]);
		expect(rows[0]?.path).toBe("src/lib/files");
	});

	test("stops folding where a directory holds files of its own", () => {
		const rows = tree(["src/lib/row.ts", "src/app.ts"]);

		expect(layout(rows)).toEqual(["src/", "  lib/", "    src/lib/row.ts", "  src/app.ts"]);
	});

	test("sorts names naturally, case only breaking ties", () => {
		const rows = tree(["v9.ts", "Outline.tsx", "v10.ts", "Beta.ts", "lineStats.ts", "alpha.ts"]);

		expect(layout(rows)).toEqual([
			"alpha.ts",
			"Beta.ts",
			"lineStats.ts",
			"Outline.tsx",
			"v9.ts",
			"v10.ts",
		]);
	});

	test("a collapsed directory hides its rows but keeps its own", () => {
		const rows = tree(["src/ui/row.ts", "src/app.ts", "readme.md"], { src: true });

		expect(layout(rows)).toEqual(["src/", "readme.md"]);
	});

	test("a directory row carries every file below it, expansion order", () => {
		const rows = tree(["src/ui/row.ts", "src/app.ts"], { src: true });

		expect(rows[0]).toMatchObject({
			_tag: "Directory",
			path: "src",
			filePaths: ["src/ui/row.ts", "src/app.ts"],
		});
	});
});

describe("selectedFilePath", () => {
	const rows = tree(["src/ui/row.ts", "src/app.ts"], { src: true });

	test("resolves a directory to the first file below it", () => {
		expect(selectedFilePath(rows, "src")).toBe("src/ui/row.ts");
	});

	test("passes a file, an unknown path, and no selection through", () => {
		expect(selectedFilePath(tree(["a.ts"]), "a.ts")).toBe("a.ts");
		expect(selectedFilePath(rows, "filtered-out.ts")).toBe("filtered-out.ts");
		expect(selectedFilePath(rows, null)).toBeNull();
	});
});

describe("parentDirectoryRow", () => {
	const rows = tree(["src/ui/row.ts", "src/app.ts", "readme.md"]);

	test("finds the directory a row sits in, skipping deeper rows", () => {
		expect(parentDirectoryRow(rows, layout(rows).indexOf("  src/app.ts"))).toMatchObject({
			path: "src",
		});
	});

	test("is null at the top level", () => {
		expect(parentDirectoryRow(rows, 0)).toBeNull();
		expect(parentDirectoryRow(rows, rows.length - 1)).toBeNull();
	});
});
