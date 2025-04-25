export function splitFilePath(filepath: string): { filename: string; path: string } {
	const parts = filepath.split('/');
	if (parts.length === 0) {
		return { filename: '', path: '' };
	}

	const filename = parts.at(-1) ?? '';
	const path = parts.slice(0, -1).join('/');

	return { filename, path };
}
