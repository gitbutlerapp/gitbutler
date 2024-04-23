export function isDefined<T>(file: T | undefined | null): file is T {
	return file !== undefined;
}

export function notNull<T>(file: T | undefined | null): file is T {
	return file !== null;
}
