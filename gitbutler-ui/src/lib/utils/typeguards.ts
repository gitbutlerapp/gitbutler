export function isDefined<T>(file: T | undefined): file is T {
	return file !== undefined;
}
