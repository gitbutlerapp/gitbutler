/**
 * Represents a selected file, can typically have a context menu
 * and/or be dragged.
 */
export type SelectedFile = {
	path: string;
	commitId?: string;
};

export function key(path: string, commitId?: string) {
	return `${path}:${commitId}`;
}

export function splitKey(key: string): SelectedFile {
	const [path, commitId] = key.split(':');
	if (commitId === 'undefined') {
		return { path: path! };
	}
	return { path: path!, commitId };
}
