import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

export default defineConfig({
	root: currentDirPath,
	plugins: [react()],
	build: {
		outDir: '../dist/ui',
		emptyOutDir: true
	},
	resolve: {
		alias: {
			'@': path.resolve(currentDirPath, './src')
		}
	},
	server: {
		port: 5173,
		strictPort: true
	}
});
