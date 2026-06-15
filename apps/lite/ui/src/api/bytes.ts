// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
export const decodeBytes = (b: Array<number>): string =>
	new TextDecoder().decode(Uint8Array.from(b));

export const encodeBytes = (s: string): Array<number> => Array.from(new TextEncoder().encode(s));

export const bytesEqual = (left: Array<number>, right: Array<number>): boolean =>
	left === right ||
	(left.length === right.length && left.every((value, index) => value === right[index]));
