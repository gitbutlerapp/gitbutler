/**
 * Ensures that a value is not null or undefined, throwing an error if it is.
 */
export function ensureValue<T>(value: T | null | undefined): T {
	if (value === null || value === undefined) {
		const message = `Expected value but got ${value === null ? 'null' : 'undefined'}`;
		const error = new Error(message);
		Error.captureStackTrace(error, ensureValue);
		throw error;
	}
	return value;
}
