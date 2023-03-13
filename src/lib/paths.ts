export function shortPath(path: string, max = 3) {
	if (path.length < 30) {
		return path;
	}
	const pathParts = path.split('/');
	const file = pathParts.pop();
	if (pathParts.length > 0) {
		const pp = pathParts.map((p) => p.slice(0, max)).join('/');
		return `${pp}/${file}`;
	}
	return file;
}
