import { expect, test } from 'vitest';
import { File } from '$lib/vbranches/types';
import { filesToFileTree } from './filetree';
import { plainToInstance } from 'class-transformer';

test('creates a file tree', () => {
	const files = [
		plainToInstance(File, {
			id: '1234',
			path: 'test/foo.py',
			hunks: [],
			expanded: true,
			modifiedAt: Date.now(),
			conflicted: false,
			content: undefined,
			binary: false
		}),
		plainToInstance(File, {
			id: '1234',
			path: 'test/bar.rs',
			hunks: [],
			expanded: true,
			modifiedAt: Date.now(),
			conflicted: false,
			content: undefined,
			binary: false
		}),
		plainToInstance(File, {
			id: '1234',
			path: 'src/hello/world.txt',
			hunks: [],
			expanded: true,
			modifiedAt: Date.now(),
			conflicted: false,
			content: undefined,
			binary: false
		})
	];
	const fileTree = filesToFileTree(files);
	expect(fileTree).toHaveLength(2);

	expect(fileTree[0].name).toEqual('test');
	expect(fileTree[0].children[0].name).toEqual('foo.py');
	expect(fileTree[0].children[0].file).toEqual(files[0]);
	expect(fileTree[0].children[1].name).toEqual('bar.rs');
	expect(fileTree[0].children[1].file).toEqual(files[1]);

	expect(fileTree[1].name).toEqual('src');
	expect(fileTree[1].children[0].name).toEqual('hello');
	expect(fileTree[1].children[0].children[0].name).toEqual('world.txt');
	expect(fileTree[1].children[0].children[0].file).toEqual(files[2]);
});
