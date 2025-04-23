/**
 * It's easier to understand a hierarchical structure than a flat list.
 *
 * This module provides support for tranforming a list of files into a
 * hirerarchical structure for easy rendering.
 *
 * Note: This is a V3 replacement for `fileTree.ts`, the main change
 * being type change from `AnyFile` to `TreeChange`.
 */

import type { TreeChange } from '$lib/hunks/change';

export interface TreeNode {
	name: string;
	file?: TreeChange;
	children: TreeNode[];
	parent?: TreeNode;
}

function createNode(acc: TreeNode, pathParts: string[]) {
	if (pathParts.length === 0) {
		return acc;
	}

	const node = acc.children?.find((f) => f.name === pathParts[0]);
	if (node) return createNode(node, pathParts.slice(1));

	const newDir = { name: pathParts[0] ? pathParts[0] : '', children: [], parent: acc };
	acc.children.push(newDir);

	return createNode(newDir, pathParts.slice(1));
}

export function sortChildren(node: TreeNode) {
	node.children.sort((a, b) => {
		if (a.file && !b.file) {
			return 1;
		} else if (!a.file && b.file) {
			return -1;
		} else {
			return a.name < b.name ? -1 : 1;
		}
	});
	for (const child of node.children) {
		sortChildren(child);
	}
}

export function filesToFileTree(files: TreeChange[]): TreeNode {
	const acc: TreeNode = { name: 'root', children: [] };
	files.forEach((f) => {
		const pathParts = f.path.split('/');
		const node = createNode(acc, pathParts);
		node.file = f;
	});
	sortChildren(acc);
	return acc;
}

function fileTreeToList(node: TreeNode): TreeChange[] {
	const list: TreeChange[] = [];
	if (node.file) list.push(node.file);
	node.children.forEach((child) => {
		list.push(...fileTreeToList(child));
	});
	return list;
}

// Sorts a file list the same way it is sorted in a file tree
export function sortLikeFileTree(files: TreeChange[]): TreeChange[] {
	return fileTreeToList(filesToFileTree(files));
}
