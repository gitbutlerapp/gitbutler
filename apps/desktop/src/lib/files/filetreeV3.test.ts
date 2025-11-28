import {
	abbreviateFolders,
	changesToFileTree,
	countLeafNodes,
	getAllChanges,
	nodePath,
	sortLikeFileTree,
	type TreeNode
} from '$lib/files/filetreeV3';
import { describe, expect, it } from 'vitest';

import type { TreeChange } from '$lib/hunks/change';

/** Helper to create a minimal TreeChange for testing */
function createTreeChange(path: string): TreeChange {
	return {
		path,
		pathBytes: Array.from(new TextEncoder().encode(path)),
		status: {
			type: 'Addition',
			subject: {
				state: { id: 'test-id', kind: 'Blob' },
				isUntracked: true
			}
		}
	};
}

describe('changesToFileTree', () => {
	it('should handle empty file list', () => {
		const tree = changesToFileTree([]);
		expect(tree.kind).toBe('dir');
		expect(tree.name).toBe('root');
		expect(tree.children).toEqual([]);
	});

	it('should handle single file at root', () => {
		const changes = [createTreeChange('file.txt')];
		const tree = changesToFileTree(changes);

		expect(tree.children.length).toBe(1);
		expect(tree.children[0]?.kind).toBe('file');
		expect(tree.children[0]?.name).toBe('file.txt');
	});

	it('should handle single file in nested directory', () => {
		const changes = [createTreeChange('src/lib/file.txt')];
		const tree = changesToFileTree(changes);

		expect(tree.children.length).toBe(1);
		const src = tree.children[0];
		expect(src?.kind).toBe('dir');
		expect(src?.name).toBe('src');
		expect(src?.children.length).toBe(1);

		const lib = src?.children[0];
		expect(lib?.kind).toBe('dir');
		expect(lib?.name).toBe('lib');
		expect(lib?.children.length).toBe(1);

		const file = lib?.children[0];
		expect(file?.kind).toBe('file');
		expect(file?.name).toBe('file.txt');
	});

	it('should handle multiple files in same directory', () => {
		const changes = [
			createTreeChange('src/a.txt'),
			createTreeChange('src/b.txt'),
			createTreeChange('src/c.txt')
		];
		const tree = changesToFileTree(changes);

		expect(tree.children.length).toBe(1);
		const src = tree.children[0];
		expect(src?.kind).toBe('dir');
		expect(src?.name).toBe('src');
		expect(src?.children.length).toBe(3);

		// Files should be sorted alphabetically
		expect(src?.children[0]?.name).toBe('a.txt');
		expect(src?.children[1]?.name).toBe('b.txt');
		expect(src?.children[2]?.name).toBe('c.txt');
	});

	it('should sort directories before files', () => {
		const changes = [
			createTreeChange('a.txt'),
			createTreeChange('z/file.txt'),
			createTreeChange('b.txt')
		];
		const tree = changesToFileTree(changes);

		expect(tree.children.length).toBe(3);
		// Directory 'z' should come before files 'a.txt' and 'b.txt'
		expect(tree.children[0]?.kind).toBe('dir');
		expect(tree.children[0]?.name).toBe('z');
		expect(tree.children[1]?.kind).toBe('file');
		expect(tree.children[1]?.name).toBe('a.txt');
		expect(tree.children[2]?.kind).toBe('file');
		expect(tree.children[2]?.name).toBe('b.txt');
	});

	it('should preserve original index on file nodes', () => {
		const changes = [
			createTreeChange('z.txt'),
			createTreeChange('a.txt'),
			createTreeChange('m.txt')
		];
		const tree = changesToFileTree(changes);

		// Files are sorted by name, but should retain original index
		expect(tree.children.length).toBe(3);
		const aFile = tree.children[0] as TreeNode & { index: number };
		const mFile = tree.children[1] as TreeNode & { index: number };
		const zFile = tree.children[2] as TreeNode & { index: number };

		expect(aFile.name).toBe('a.txt');
		expect(aFile.index).toBe(1); // Was at index 1 in original array

		expect(mFile.name).toBe('m.txt');
		expect(mFile.index).toBe(2); // Was at index 2 in original array

		expect(zFile.name).toBe('z.txt');
		expect(zFile.index).toBe(0); // Was at index 0 in original array
	});

	it('should handle many files efficiently', () => {
		// This test verifies the O(n log n) performance fix
		// With the old O(n²) algorithm, 10000 files would be extremely slow
		const changes: TreeChange[] = [];
		for (let i = 0; i < 10000; i++) {
			changes.push(createTreeChange(`dir/${i}.txt`));
		}

		const start = performance.now();
		const tree = changesToFileTree(changes);
		const elapsed = performance.now() - start;

		expect(tree.children.length).toBe(1);
		const dir = tree.children[0];
		expect(dir?.children.length).toBe(10000);

		// Should complete in under 1 second (with O(n²) it would take minutes)
		expect(elapsed).toBeLessThan(1000);
	});

	it('should handle deeply nested paths', () => {
		const changes = [createTreeChange('a/b/c/d/e/f/g/h/i/j/file.txt')];
		const tree = changesToFileTree(changes);

		let current: TreeNode | undefined = tree.children[0];
		const names = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'file.txt'];

		for (const name of names) {
			expect(current?.name).toBe(name);
			current = current?.children[0];
		}
	});
});

describe('abbreviateFolders', () => {
	it('should abbreviate single-child directory chains', () => {
		const changes = [createTreeChange('a/b/c/file.txt')];
		const tree = changesToFileTree(changes);
		const abbreviated = abbreviateFolders(tree);

		// The chain a/b/c should be collapsed into a single node
		expect(abbreviated.children.length).toBe(1);
		const dir = abbreviated.children[0];
		expect(dir?.kind).toBe('dir');
		expect(dir?.name).toBe('a/b/c');
		expect(dir?.children.length).toBe(1);
		expect(dir?.children[0]?.name).toBe('file.txt');
	});

	it('should not abbreviate directories with multiple children', () => {
		const changes = [createTreeChange('a/file1.txt'), createTreeChange('a/file2.txt')];
		const tree = changesToFileTree(changes);
		const abbreviated = abbreviateFolders(tree);

		expect(abbreviated.children.length).toBe(1);
		const dir = abbreviated.children[0];
		expect(dir?.name).toBe('a');
		expect(dir?.children.length).toBe(2);
	});
});

describe('countLeafNodes', () => {
	it('should count files correctly', () => {
		const changes = [
			createTreeChange('a/file1.txt'),
			createTreeChange('b/file2.txt'),
			createTreeChange('file3.txt')
		];
		const tree = changesToFileTree(changes);
		expect(countLeafNodes(tree)).toBe(3);
	});
});

describe('nodePath', () => {
	it('should return correct path for nested file', () => {
		const changes = [createTreeChange('a/b/file.txt')];
		const tree = changesToFileTree(changes);

		// Navigate to the file
		const a = tree.children[0]!;
		const b = a.children[0]!;
		const file = b.children[0]!;

		expect(nodePath(file)).toBe('a/b/file.txt');
	});

	it('should return empty string for root', () => {
		const tree = changesToFileTree([]);
		expect(nodePath(tree)).toBe('');
	});
});

describe('getAllChanges', () => {
	it('should return all changes from tree', () => {
		const changes = [
			createTreeChange('a/file1.txt'),
			createTreeChange('b/file2.txt'),
			createTreeChange('file3.txt')
		];
		const tree = changesToFileTree(changes);
		const allChanges = getAllChanges(tree);

		expect(allChanges.length).toBe(3);
		// Note: order depends on sort order
		expect(allChanges.map((c) => c.path).sort()).toEqual(
			['a/file1.txt', 'b/file2.txt', 'file3.txt'].sort()
		);
	});
});

describe('sortLikeFileTree', () => {
	it('should sort files with deeper paths first', () => {
		const changes = [
			createTreeChange('a.txt'),
			createTreeChange('dir/b.txt'),
			createTreeChange('z.txt')
		];
		const sorted = sortLikeFileTree(changes);

		// Subdirectories first (deeper paths), then files
		expect(sorted[0]?.path).toBe('dir/b.txt');
	});
});
