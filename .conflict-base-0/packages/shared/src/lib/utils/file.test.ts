import * as FileUtils from '$lib/utils/file';
import { assert, test, describe } from 'vitest';

describe('file utils', () => {
	describe('isWindowsPath', () => {
		test('returns true for windows paths', () => {
			assert.isTrue(FileUtils.isWindowsPath('C:\\Users\\test'));
			assert.isTrue(FileUtils.isWindowsPath('\\\\network\\share'));
		});

		test('returns false for unix paths', () => {
			assert.isFalse(FileUtils.isWindowsPath('/usr/local/bin'));
			assert.isFalse(FileUtils.isWindowsPath('./relative/path'));
			assert.isFalse(FileUtils.isWindowsPath('./relative/path/my\\ file'));
		});
	});

	describe('getSeparator', () => {
		test('returns windows separator for windows paths', () => {
			assert.equal(FileUtils.getSeparator('C:\\Users\\test'), '\\');
		});

		test('returns unix separator for unix paths', () => {
			assert.equal(FileUtils.getSeparator('/usr/local/bin'), '/');
		});
	});

	describe('splitPath', () => {
		test('splits windows paths correctly', () => {
			assert.deepEqual(FileUtils.splitPath('C:\\Users\\test\\file.txt'), [
				'C:',
				'Users',
				'test',
				'file.txt'
			]);
		});

		test('splits unix paths correctly', () => {
			assert.deepEqual(FileUtils.splitPath('/usr/local/file.txt'), [
				'',
				'usr',
				'local',
				'file.txt'
			]);
		});
	});

	describe('getFilePathInfo', () => {
		test('returns correct info for windows path', () => {
			const info = FileUtils.getFilePathInfo('C:\\Users\\test\\file.txt');
			assert.deepEqual(info, {
				fileName: 'file.txt',
				extension: 'txt',
				directoryPath: 'C:\\Users\\test'
			});
		});

		test('returns correct info for unix path', () => {
			const info = FileUtils.getFilePathInfo('/usr/local/file.txt');
			assert.deepEqual(info, {
				fileName: 'file.txt',
				extension: 'txt',
				directoryPath: '/usr/local'
			});
		});

		test('handles paths without extension', () => {
			const info = FileUtils.getFilePathInfo('/usr/local/filename');
			assert.deepEqual(info, {
				fileName: 'filename',
				extension: '',
				directoryPath: '/usr/local'
			});
		});

		test('handles paths with multiple extensions', () => {
			const info = FileUtils.getFilePathInfo('/usr/local/file.tar.gz');
			assert.deepEqual(info, {
				fileName: 'file.tar.gz',
				extension: 'gz',
				directoryPath: '/usr/local'
			});
		});

		test('handles paths with no directory', () => {
			const info = FileUtils.getFilePathInfo('file.txt');
			assert.deepEqual(info, {
				fileName: 'file.txt',
				extension: 'txt',
				directoryPath: ''
			});
		});

		test('returns undefined for empty path', () => {
			assert.isUndefined(FileUtils.getFilePathInfo(''));
		});
	});
});
