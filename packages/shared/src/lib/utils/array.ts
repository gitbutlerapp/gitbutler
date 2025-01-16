export function deduplicate<T>(array: T[]): T[] {
	return Array.from(new Set(array));
}
