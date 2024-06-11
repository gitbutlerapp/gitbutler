export function hashCode(s: string) {
	let hash = 0;
	let chr;
	let i;

	if (s.length === 0) return hash.toString();
	for (i = 0; i < s.length; i++) {
		chr = s.charCodeAt(i);
		hash = (hash << 5) - hash + chr;
		hash |= 0; // Convert to 32bit integer
	}
	return hash.toString();
}

export function isChar(char: string) {
	return char.length === 1;
}

export function isStr(s: unknown): s is string {
	return typeof s === 'string';
}
