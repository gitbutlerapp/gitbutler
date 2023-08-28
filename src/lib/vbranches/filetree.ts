/**
 * It's easier to understand a hierarchical structure than a flat list.
 *
 * This module provides support for tranforming a list of files into a
 * hirerarchical structure for easy rendering.
 */
import type { File } from './types';

export interface TreeNode {
	name: string;
	file?: File;
	children: TreeNode[];
}

function createNode(acc: TreeNode, pathParts: string[]) {
	if (pathParts.length == 0) return acc;

	const node = acc.children?.find((f) => f.name == pathParts[0]);
	if (node) return createNode(node, pathParts.slice(1));

	const newDir = { name: pathParts[0], children: [] };
	acc.children.push(newDir);

	return createNode(newDir, pathParts.slice(1));
}

export function filesToFileTree(files: File[]): TreeNode {
	const acc: TreeNode = { name: 'root', children: [] };
	files.forEach((f) => {
		const pathParts = f.path.split('/');
		const node = createNode(acc, pathParts);
		node.file = f;
	});
	return acc;
}
