import { convertRemoteToWebUrl, getEditorUri } from '$lib/utils/url';
import { describe, expect, test } from 'vitest';

describe.concurrent('cleanUrl', () => {
	test('it should handle url starts with http(s)?', () => {
		expect(convertRemoteToWebUrl('https://github.com/user/repo.git')).toEqual(
			'https://github.com/user/repo'
		);
	});

	test('it should handle complete ssh url with domain name', () => {
		expect(convertRemoteToWebUrl('ssh://git@github.com:22/user/repo.git')).toEqual(
			'https://github.com/user/repo'
		);
	});

	test('it should handle complete ssh url with ip', () => {
		expect(convertRemoteToWebUrl('ssh://git@192.168.1.1:22/user/repo.git')).toEqual(
			'http://192.168.1.1/user/repo'
		);
	});

	test('it should handle SCP-like url', () => {
		expect(convertRemoteToWebUrl('git@github.com:user/repo.git')).toEqual(
			'https://github.com/user/repo'
		);
	});
});

describe.concurrent('getEditorUri', () => {
	test('it should handle editor path with no search params', () => {
		expect(getEditorUri({ schemeId: 'vscode', path: ['/path', 'to', 'file'] })).toEqual(
			'vscode://file/path/to/file'
		);
	});

	test('it should handle editor path with search params', () => {
		expect(
			getEditorUri({
				schemeId: 'vscode',
				path: ['/path', 'to', 'file'],
				searchParams: { something: 'cool' }
			})
		).toEqual('vscode://file/path/to/file?something=cool');
	});

	test('it should handle editor path with search params with special characters', () => {
		expect(
			getEditorUri({
				schemeId: 'vscode',
				path: ['/path', 'to', 'file'],
				searchParams: {
					search: 'hello world',
					what: 'bye-&*%*\\ded-yeah'
				}
			})
		).toEqual('vscode://file/path/to/file?search=hello+world&what=bye-%26*%25*%5Cded-yeah');
	});

	test('it should handle editor path with search params with line number', () => {
		expect(
			getEditorUri({
				schemeId: 'vscode',
				path: ['/path', 'to', 'file'],
				line: 10
			})
		).toEqual('vscode://file/path/to/file:10');
	});

	test('it should handle editor path with search params with line and column number', () => {
		expect(
			getEditorUri({
				schemeId: 'vscode',
				path: ['/path', 'to', 'file'],
				searchParams: {
					another: 'thing'
				},
				line: 10,
				column: 20
			})
		).toEqual('vscode://file/path/to/file:10:20?another=thing');
	});

	test('it should ignore the column if there is no line number', () => {
		expect(
			getEditorUri({
				schemeId: 'vscode',
				path: ['/path', 'to', 'file'],
				searchParams: {
					another: 'thing'
				},
				column: 20
			})
		).toEqual('vscode://file/path/to/file?another=thing');
	});
});
