/**
 * Not undefined and not null. This is less prone for errors than checking undefined
 * and not null separately.
 */
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

/**
 * Checks if the provided value is an Error.
 * @param value - The value to be checked.
 * @returns A boolean indicating whether the value is an Error.
 */
export function isError(value: unknown): value is Error {
	return value instanceof Error;
}
