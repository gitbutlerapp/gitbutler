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

/**
 * Checks if a string is a single character.
 *
 * @param char - The string to check.
 * @returns `true` if the string is a single character, `false` otherwise.
 */
export function isChar(char: string) {
	return char.length === 1;
}

/**
 * Checks if a value is a string.
 *
 * @param s - The value to check.
 * @returns A boolean indicating whether the value is a string.
 */
export function isStr(s: unknown): s is string {
	return typeof s === 'string';
}
