import { defineConfig } from 'vitest/config';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

export default defineConfig({
	test: {
		environment: 'jsdom'
	},
	resolve: {
		alias: {
			'@': path.resolve(currentDirPath, './src')
		}
	}
});
