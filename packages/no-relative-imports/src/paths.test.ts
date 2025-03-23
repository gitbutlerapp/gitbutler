import { ConfigPaths, PathEntry } from './paths.js';
import { describe, expect, test } from 'vitest';

describe('ConfigPaths', () => {
	test('Matching non globbed path to be aliased', () => {
		const configPaths = new ConfigPaths();

		configPaths.pushPaths('/a/b/c', {
			foo: ['./bar']
		});

		expect(configPaths.tryAliasImport('/a/b/c/bar')).toBe('foo');
		expect(configPaths.tryAliasImport('/a/b/c/bar/xyz')).toBe(undefined);
	});

	test('Matching globbed path to be aliased', () => {
		const configPaths = new ConfigPaths();

		configPaths.pushPaths('/a/b/c', {
			'foo/*': ['./bar/*']
		});

		expect(configPaths.tryAliasImport('/a/b/c/bar')).toBe(undefined);
		expect(configPaths.tryAliasImport('/a/b/c/bar/xyz')).toBe('foo/xyz');
	});

	test('Complex globs are ignored', () => {
		const configPaths = new ConfigPaths();

		configPaths.pushPaths('/a/b/c', {
			'foo/*/bar': ['./bar/*'],
			'foo/**': ['./qux/*'],
			foo: ['./foo/**'],
			bar: ['./foo/*/qux']
		});

		expect(configPaths.tryAliasImport('/a/b/c/bar/xyz')).toBe(undefined);
		expect(configPaths.tryAliasImport('/a/b/c/qux/xyz')).toBe(undefined);
		expect(configPaths.tryAliasImport('/a/b/c/foo/xyz')).toBe(undefined);
		expect(configPaths.tryAliasImport('/a/b/c/foo/xyz/qux')).toBe(undefined);
	});
});

describe('PathEntry', () => {
	test('Matching globs with non-glob keys get converted to key', () => {
		const pathEntry = new PathEntry('/a/b/c', '$', './foo/*');

		expect(pathEntry.tryAliasImport('/a/b/c/foo/xyz')).toBe('$');
		expect(pathEntry.tryAliasImport('/a/b/c/foo')).toBe(undefined);
	});

	test('Matching globs with glob keys have start replaced with key', () => {
		const pathEntry = new PathEntry('/a/b/c', '$/*', './foo/*');

		expect(pathEntry.tryAliasImport('/a/b/c/foo/xyz')).toBe('$/xyz');
		expect(pathEntry.tryAliasImport('/a/b/c/foo')).toBe(undefined);
	});

	test('Glob keys dont require forward slashes', () => {
		const pathEntry = new PathEntry('/a/b/c', '$*', './foo/*');

		expect(pathEntry.tryAliasImport('/a/b/c/foo/xyz')).toBe('$xyz');
		expect(pathEntry.tryAliasImport('/a/b/c/foo')).toBe(undefined);
	});

	test('Glob values dont require forward slashes', () => {
		const pathEntry = new PathEntry('/a/b/c', '$*', './foo*');

		expect(pathEntry.tryAliasImport('/a/b/c/foo/xyz')).toBe('$/xyz');
		expect(pathEntry.tryAliasImport('/a/b/c/foo')).toBe('$');
	});
});
