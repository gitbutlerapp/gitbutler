export function getFilename(path: string) {
	return path.split(/[/\\]/).pop();
}

export function getDir(path: string) {
	const parts = path.split(/[/\\]/);
	parts.pop();
	return parts.join(path.includes('\\') ? '\\' : '/');
}
