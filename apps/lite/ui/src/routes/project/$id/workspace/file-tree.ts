/**
 * @file Folder-shaped layout for the file lists.
 *
 * Rows come out flat and in render order, one array for both display modes:
 * the list is the tree with every path kept whole at depth zero. Flat keeps
 * rendering a single `map`, and lets the navigation index stay a list of paths
 * — a directory path and a file path can never collide, so one string still
 * identifies any row.
 */

import { buildIndexByKey, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { compareFilePaths } from "#ui/file-order.ts";

export type FileDisplayMode = "list" | "tree";

export type FileTreeRow<T> = {
	path: string;
	/** Indent depth, counted from zero. Every list-mode row sits at zero. */
	depth: number;
} & (
	| {
			_tag: "Directory";
			/** The last path segment, or several joined when a sole-child chain was folded in. */
			name: string;
			/** Every file below this directory, in the order expanding it would reveal them. */
			filePaths: Array<string>;
	  }
	| { _tag: "File"; item: T }
);

type Directory<T> = {
	directories: Map<string, Directory<T>>;
	items: Array<T>;
};

const emptyDirectory = <T>(): Directory<T> => ({ directories: new Map(), items: [] });

const insert = <T extends { path: string }>(root: Directory<T>, item: T): void => {
	const segments = item.path.split("/");
	let directory = root;

	for (const segment of segments.slice(0, -1)) {
		let child = directory.directories.get(segment);
		if (child === undefined) {
			child = emptyDirectory();
			directory.directories.set(segment, child);
		}
		directory = child;
	}

	directory.items.push(item);
};

type NamedDirectory<T> = { name: string; directory: Directory<T> };

/** The one directory a directory holds, when that is all it holds. */
const soleChild = <T>(directory: Directory<T>): NamedDirectory<T> | null => {
	if (directory.items.length > 0 || directory.directories.size !== 1) return null;

	const [entry] = directory.directories;
	return entry === undefined ? null : { name: entry[0], directory: entry[1] };
};

/**
 * Fold a chain of directories that each hold nothing but the next one into a
 * single row, so `src` › `lib` › `files` arrives as `src/lib/files` rather than
 * three rows deep with nothing to choose between them.
 */
const foldSoleChildren = <T>(name: string, directory: Directory<T>): NamedDirectory<T> => {
	let folded: NamedDirectory<T> = { name, directory };

	for (let child = soleChild(folded.directory); child !== null; child = soleChild(folded.directory))
		folded = { name: `${folded.name}/${child.name}`, directory: child.directory };

	return folded;
};

/** One directory's worth of rows, and every file path below it. */
type Collected<T> = { rows: Array<FileTreeRow<T>>; filePaths: Array<string> };

const collectRows = <T extends { path: string }>({
	directory,
	prefix,
	depth,
	collapsedDirectories,
}: {
	directory: Directory<T>;
	prefix: string;
	depth: number;
	collapsedDirectories: Record<string, true>;
}): Collected<T> => {
	const rows: Array<FileTreeRow<T>> = [];
	const filePaths: Array<string> = [];

	for (const [name, child] of directory.directories) {
		const folded = foldSoleChildren(name, child);
		const { name: foldedName, directory: foldedDirectory } = folded;
		const path = prefix === "" ? foldedName : `${prefix}/${foldedName}`;
		// Walked whether or not it is collapsed: collapsing keeps a directory's rows
		// out of the list, but it still answers for its files at its own checkbox.
		const below = collectRows({
			directory: foldedDirectory,
			prefix: path,
			depth: depth + 1,
			collapsedDirectories,
		});

		rows.push({ _tag: "Directory", path, name: foldedName, depth, filePaths: below.filePaths });
		if (collapsedDirectories[path] !== true) rows.push(...below.rows);
		filePaths.push(...below.filePaths);
	}

	for (const item of directory.items) {
		rows.push({ _tag: "File", path: item.path, item, depth });
		filePaths.push(item.path);
	}

	return { rows, filePaths };
};

export const buildFileTreeRows = <T extends { path: string }>({
	items,
	mode,
	collapsedDirectories,
}: {
	items: Array<T>;
	mode: FileDisplayMode;
	collapsedDirectories: Record<string, true>;
}): Array<FileTreeRow<T>> => {
	const orderedItems = items.toSorted((a, b) => compareFilePaths(a.path, b.path));

	if (mode === "list")
		return orderedItems.map((item) => ({ _tag: "File", path: item.path, item, depth: 0 }));

	const root = emptyDirectory<T>();
	for (const item of orderedItems) insert(root, item);

	return collectRows({ directory: root, prefix: "", depth: 0, collapsedDirectories }).rows;
};

export const fileTreeNavigationIndex = <T>(
	rows: Array<FileTreeRow<T>>,
): NavigationIndex<string> => {
	const items = rows.map((row) => row.path);
	return { items, indexByKey: buildIndexByKey(items, (path) => path) };
};

/**
 * The file a selected row stands for: itself, or — for a directory — the first
 * file beneath it, so the diff has something to show while the cursor rests on
 * a folder. A path the rows don't hold is passed through, since the caller's
 * own list may reach further than the filtered rows do.
 */
export const selectedFilePath = <T>(
	rows: Array<FileTreeRow<T>>,
	selection: string | null,
): string | null => {
	if (selection === null) return null;

	const row = rows.find((row) => row.path === selection);
	if (row === undefined) return selection;

	return row._tag === "File" ? row.path : (row.filePaths[0] ?? null);
};

/** The directory row a row sits in, or `null` at the top level. */
export const parentDirectoryRow = <T>(
	rows: Array<FileTreeRow<T>>,
	index: number,
): Extract<FileTreeRow<T>, { _tag: "Directory" }> | null => {
	const row = rows[index];
	if (row === undefined || row.depth === 0) return null;

	for (let candidateIndex = index - 1; candidateIndex >= 0; candidateIndex--) {
		const candidate = rows[candidateIndex];
		if (candidate !== undefined && candidate.depth === row.depth - 1)
			return candidate._tag === "Directory" ? candidate : null;
	}

	return null;
};
