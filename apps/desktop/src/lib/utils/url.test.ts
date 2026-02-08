import { getEditorUri } from '$lib/utils/url';
import { expect, test, describe } from 'vitest';

describe.concurrent('getEditorUri', () => {
	test('generates VSCode URI correctly', () => {
		const uri = getEditorUri({
			schemeId: 'vscode',
			path: ['/home/user/project', 'src/file.ts']
		});

		expect(uri).toBe('vscode://file/home/user/project/src/file.ts');
	});

	test('generates VSCode URI with line number', () => {
		const uri = getEditorUri({
			schemeId: 'vscode',
			path: ['/home/user/project', 'src/file.ts'],
			line: 10
		});

		expect(uri).toBe('vscode://file/home/user/project/src/file.ts:10');
	});

	test('generates VSCode URI with line and column', () => {
		const uri = getEditorUri({
			schemeId: 'vscode',
			path: ['/home/user/project', 'src/file.ts'],
			line: 10,
			column: 5
		});

		expect(uri).toBe('vscode://file/home/user/project/src/file.ts:10:5');
	});

	test('generates JetBrains IDE URI correctly', () => {
		const uri = getEditorUri({
			schemeId: 'jetbrains',
			path: ['/home/user/project', 'src/file.ts']
		});

		expect(uri).toBe(
			'jetbrains://idea/navigate/reference?project=%2Fhome%2Fuser%2Fproject&path=src%2Ffile.ts'
		);
	});

	test('generates JetBrains IDE URI with line number', () => {
		const uri = getEditorUri({
			schemeId: 'jetbrains',
			path: ['/home/user/project', 'src/file.ts'],
			line: 10
		});

		expect(uri).toBe(
			'jetbrains://idea/navigate/reference?project=%2Fhome%2Fuser%2Fproject&path=src%2Ffile.ts%3A10'
		);
	});

	test('generates JetBrains IDE URI with line and column', () => {
		const uri = getEditorUri({
			schemeId: 'jetbrains',
			path: ['/home/user/project', 'src/file.ts'],
			line: 10,
			column: 5
		});

		expect(uri).toBe(
			'jetbrains://idea/navigate/reference?project=%2Fhome%2Fuser%2Fproject&path=src%2Ffile.ts%3A10%3A5'
		);
	});

	test('generates JetBrains IDE URI with nested file path', () => {
		const uri = getEditorUri({
			schemeId: 'jetbrains',
			path: ['/home/user/project', 'src/components/Button.tsx'],
			line: 25,
			column: 10
		});

		expect(uri).toBe(
			'jetbrains://idea/navigate/reference?project=%2Fhome%2Fuser%2Fproject&path=src%2Fcomponents%2FButton.tsx%3A25%3A10'
		);
	});

	test('generates Zed URI correctly', () => {
		const uri = getEditorUri({
			schemeId: 'zed',
			path: ['/home/user/project', 'src/file.ts']
		});

		expect(uri).toBe('zed://file/home/user/project/src/file.ts');
	});

	test('generates Cursor URI with line and column', () => {
		const uri = getEditorUri({
			schemeId: 'cursor',
			path: ['/home/user/project', 'src/file.ts'],
			line: 42,
			column: 7
		});

		expect(uri).toBe('cursor://file/home/user/project/src/file.ts:42:7');
	});
});
