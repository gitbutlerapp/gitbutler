// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
export const decodeRefName = (fullNameBytes: Array<number>): string =>
	new TextDecoder().decode(Uint8Array.from(fullNameBytes));

export const encodeRefName = (fullName: string): Array<number> =>
	Array.from(new TextEncoder().encode(fullName));

export const refNamesEqual = (left: Array<number>, right: Array<number>): boolean =>
	left === right ||
	(left.length === right.length && left.every((value, index) => value === right[index]));
