import { LocalFile } from '$lib/files/file';
import { filesToFileTree } from '$lib/files/filetree';
import { plainToInstance } from 'class-transformer';
import { expect, test } from 'vitest';

test('creates a file tree', () => {
	const files = [
		plainToInstance(LocalFile, {
			id: '1234',
			path: 'test/foo.py',
			hunks: [],
			expanded: true,
			modifiedAt: Date.now(),
			conflicted: false,
			content: undefined,
			binary: false
		}),
		plainToInstance(LocalFile, {
			id: '1234',
			path: 'test/bar.rs',
			hunks: [],
			expanded: true,
			modifiedAt: Date.now(),
			conflicted: false,
			content: undefined,
			binary: false
		}),
		plainToInstance(LocalFile, {
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
	const children = fileTree.children;
	expect(children).toHaveLength(2);

	// Note that the result is sorted
	//  - folders first
	//  - then alphanumerically
	expect(children[1]?.name).toEqual('test');
	expect(children[1]?.children[0]?.name).toEqual('bar.rs');
	expect(children[1]?.children[0]?.file).toEqual(files[1]);
	expect(children[1]?.children[1]?.name).toEqual('foo.py');
	expect(children[1]?.children[1]?.file).toEqual(files[0]);

	expect(children[0]?.name).toEqual('src');
	expect(children[0]?.children[0]?.name).toEqual('hello');
	expect(children[0]?.children[0]?.children[0]?.name).toEqual('world.txt');
	expect(children[0]?.children[0]?.children[0]?.file).toEqual(files[2]);
});
