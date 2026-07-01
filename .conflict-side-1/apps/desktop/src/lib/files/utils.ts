export function getFilename(path: string) {
	return path.split(/[/\\]/).pop();
}
