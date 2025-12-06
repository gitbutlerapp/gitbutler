/**
 * It's easier to understand a hierarchical structure than a flat list.
 *
 * This module provides support for transforming a list of files into a
 * hirerarchical structure for easy rendering.
 */

import type { TreeChange } from '$lib/hunks/change';

export type TreeNode = {
	kind: 'dir' | 'file';
	name: string;
	children: TreeNode[];
	parent?: TreeNode;
} & (
	| {
			kind: 'dir';
	  }
	| {
			kind: 'file';
			index: number;
			change: TreeChange;
	  }
);

function createNode(acc: TreeNode, pathParts: string[]) {
	if (pathParts.length === 0) {
		acc.kind = 'file';
		return acc;
	}

	const node = acc.children?.find((f) => f.name === pathParts[0]);
	if (node) {
		return createNode(node, pathParts.slice(1));
	}

	const newDir: TreeNode = {
		kind: 'dir',
		name: pathParts[0] ? pathParts[0] : '',
		children: [],
		parent: acc
	};
	acc.children.push(newDir);

	return createNode(newDir, pathParts.slice(1));
}

export function sortChildren(node: TreeNode) {
	node.children.sort((a, b) => {
		if (a.kind === 'file' && b.kind === 'dir') {
			return 1;
		} else if (a.kind === 'dir' && b.kind === 'file') {
			return -1;
		} else {
			return a.name < b.name ? -1 : 1;
		}
	});
	for (const child of node.children) {
		sortChildren(child);
	}
}

export function changesToFileTree(files: TreeChange[]): TreeNode {
	const acc: TreeNode = { kind: 'dir', name: 'root', children: [] };
	files.forEach((f, index) => {
		const pathParts = f.path.split('/');
		const node = createNode(acc, pathParts);
		if (node.kind === 'file') {
			node.change = f;
			node.index = index;
		}
	});
	sortChildren(acc);
	return acc;
}

export function sortLikeFileTree(changes: TreeChange[]): TreeChange[] {
	const caseSensitive = false;
	const locale = 'en';
	const numeric = true;
	const separator = '/';

	const compareOptions: Intl.CollatorOptions = {
		sensitivity: caseSensitive ? 'case' : 'base',
		numeric: numeric,
		caseFirst: 'lower'
	};

	return changes.sort((a, b) => {
		const partsA = a.path.split(separator);
		const partsB = b.path.split(separator);

		// Compare directory by directory
		const minLength = Math.min(partsA.length, partsB.length);

		for (let i = 0; i < minLength - 1; i++) {
			const comparison = partsA[i]!.localeCompare(partsB[i]!, locale, compareOptions);
			if (comparison !== 0) {
				return comparison;
			}
		}

		// Same parent directory - subfolders first
		if (partsA.length !== partsB.length) {
			return partsB.length - partsA.length;
		}

		// Same depth, compare final component
		return partsA[partsA.length - 1]!.localeCompare(
			partsB[partsB.length - 1]!,
			locale,
			compareOptions
		);
	});
}

/**
 * Abbreviate nested folders that contain only a folder.
 *
 * Instead of this:
 * - folder
 *   - subFolder
 *     - file.txt
 *
 * We want this:
 * - folder/subFolder
 *   - file.txt
 */
export function abbreviateFolders(node: TreeNode): TreeNode {
	const newNode = { ...node };
	if (newNode.kind === 'file') {
		return newNode;
	}
	// A node without a parent is the root node. Since this node is not
	// rendered we should not try to abbreviate it.
	if (newNode.parent) {
		while (newNode.children.length === 1) {
			const grandChild = newNode.children[0]!;
			if (grandChild.kind === 'file') {
				break;
			} else {
				newNode.name = newNode.name + '/' + grandChild.name;
				newNode.children = [...grandChild.children];
			}
		}
	}
	const children = newNode.children.map((child) => abbreviateFolders(child));
	newNode.children = children;
	return newNode;
}

export function countLeafNodes(node: TreeNode): number {
	if (node.kind === 'file') {
		return 1;
	}
	return node.children.reduce((prev, curr) => prev + countLeafNodes(curr), 0);
}

export function nodePath(node: TreeNode): string {
	if (!node.parent) {
		return '';
	}
	const parentPath = nodePath(node.parent);
	return parentPath ? parentPath + '/' + node.name : node.name;
}

export function getAllChanges(node: TreeNode): TreeChange[] {
	if (node.kind === 'file') {
		return [node.change];
	}
	return node.children.reduce((prev, curr) => prev.concat(getAllChanges(curr)), [] as TreeChange[]);
}
