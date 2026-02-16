export function chunk<T>(arr: T[], size: number) {
	return Array.from({ length: Math.ceil(arr.length / size) }, (_v, i) =>
		arr.slice(i * size, i * size + size),
	);
}

export function unique<T>(arr: T[]): T[] {
	return Array.from(new Set(arr));
}
