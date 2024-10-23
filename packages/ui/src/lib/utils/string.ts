export function camelCaseToTitleCase(input: string) {
	let result = input.charAt(0).toUpperCase();
	for (let i = 1; i < input.length; i++) {
		if (
			input.charAt(i) === input.charAt(i).toUpperCase() &&
			input.charAt(i) !== input.charAt(i).toLowerCase()
		) {
			result += ' ' + input.charAt(i);
		} else {
			result += input.charAt(i);
		}
	}
	return result;
}

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

export function isWhiteSpaceString(s: string) {
	return s.trim() === '';
}

export function slugify(input: string) {
	return String(input)
		.normalize('NFKD')
		.replace(/[\u0300-\u036f]/g, '')
		.trim()
		.replace(/[^A-Za-z0-9/ -]/g, '')
		.replace(/\s+/g, '-')
		.replace(/-+/g, '-');
}
