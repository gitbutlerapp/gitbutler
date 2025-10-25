export function splitFilePath(filepath: string): { filename: string; path: string } {
	const parts = filepath.split('/');
	if (parts.length === 0) {
		return { filename: '', path: '' };
	}

	const filename = parts.at(-1) ?? '';
	const path = parts.slice(0, -1).join('/');

	return { filename, path };
}

export function abbreviatePath(path: string, maxParts: number = 4): string {
	// Split the path into parts, filtering out empty strings
	const parts = path.split('/').filter((p) => p.length > 0);

	// Handle absolute paths (starting with /)
	const isAbsolute = path.startsWith('/');

	// If we have fewer parts than the max, return as-is
	if (parts.length <= maxParts) {
		return isAbsolute ? '/' + parts.join('/') : parts.join('/');
	}

	// Keep the last maxParts elements, replace the rest with a single ".."
	const abbreviated = ['..', ...parts.slice(-maxParts)];

	return isAbsolute ? '/' + abbreviated.join('/') : abbreviated.join('/');
}
