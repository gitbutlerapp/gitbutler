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

export function hashCode(value: string) {
	let hash = 5381; // Starting with a prime number

	for (let i = 0; i < value.length; i++) {
		hash = (hash * 33) ^ value.charCodeAt(i); // Bitwise XOR and multiplication
	}

	// Convert to an unsigned 32-bit integer, then to a hexadecimal string
	return (hash >>> 0).toString(16);
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
		.replace(/[^A-Za-z0-9._/ -]/g, '')
		.replace(/\s+/g, '-')
		.replace(/-+/g, '-');
}
