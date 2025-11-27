/**
 * It's easier to understand a hierarchical structure than a flat list.
 *
 * This module provides support for tranforming a list of files into a
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

/**
 * Represents a flattened tree item that can be rendered in a virtual list.
 */
export type FlatTreeItem =
	| {
			type: 'folder';
			name: string;
			depth: number;
			node: TreeNode;
			nodeId: string;
			isExpanded: boolean;
	  }
	| {
			type: 'file';
			name: string;
			depth: number;
			change: TreeChange;
			index: number;
			nodeId: string;
	  };

/**
 * Generates a unique ID for a tree node based on its path.
 */
function getNodeId(node: TreeNode): string {
	return nodePath(node) || 'root';
}

/**
 * Flattens a tree structure into a linear array suitable for virtual scrolling.
 * Only includes items that should be visible based on expanded state.
 *
 * @param node - The tree node to flatten
 * @param expandedFolders - Set of folder IDs that are currently expanded
 * @param depth - Current depth in the tree (for indentation)
 * @returns Array of flattened items ready for rendering
 */
export function flattenTree(
	node: TreeNode,
	expandedFolders: Set<string>,
	depth: number = 0
): FlatTreeItem[] {
	const items: FlatTreeItem[] = [];

	for (const child of node.children) {
		const nodeId = getNodeId(child);

		if (child.kind === 'file') {
			items.push({
				type: 'file',
				name: child.name,
				depth,
				change: child.change,
				index: child.index,
				nodeId
			});
		} else {
			const isExpanded = expandedFolders.has(nodeId);
			items.push({
				type: 'folder',
				name: child.name,
				depth,
				node: child,
				nodeId,
				isExpanded
			});

			// Only include children if this folder is expanded
			if (isExpanded) {
				items.push(...flattenTree(child, expandedFolders, depth + 1));
			}
		}
	}

	return items;
}
