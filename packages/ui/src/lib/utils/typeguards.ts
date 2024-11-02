export function isDefined<T>(file: T | undefined | null): file is T {
	return file !== undefined && file !== null;
}

export type UnknownObject = Record<string, unknown>;

/**
 * Checks if the provided value is a non-empty object.
 * @param something - The value to be checked.
 * @returns A boolean indicating whether the value is a non-empty object.
 */
export function isNonEmptyObject(something: unknown): something is UnknownObject {
	return (
		typeof something === 'object' &&
		something !== null &&
		!Array.isArray(something) &&
		(Object.keys(something).length > 0 || Object.getOwnPropertySymbols(something).length > 0)
	);
}
