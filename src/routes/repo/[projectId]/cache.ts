import type { File } from './types';

const TRUE_KEY = '1';
const FALSE_KEY = '0';

function getKey(path: string): string {
	return `expanded:${path}`;
}

export function getExpandedWithCacheFallback(file: File) {
	if (file.expanded != undefined) {
		return file.expanded; // No need to check after initial load
	}
	const value = localStorage.getItem(`expanded:${file.path}`);
	if (value == TRUE_KEY) {
		file.expanded = true;
	} else if (value == FALSE_KEY) {
		file.expanded = false;
	}
	return file.expanded;
}

export function setExpandedWithCache(file: File, value: boolean | undefined): void {
	file.expanded = value;
	if (file.expanded != undefined) {
		localStorage.setItem(getKey(file.path), value ? TRUE_KEY : FALSE_KEY);
	} else {
		localStorage.removeItem(getKey(file.path));
	}
}
