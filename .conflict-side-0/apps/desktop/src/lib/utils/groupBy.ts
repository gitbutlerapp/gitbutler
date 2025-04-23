// TODO: Look into what browser versions we need to support and if we can use Object.groupBy
export function groupBy<T>(array: T[], callback: (item: T) => string) {
	const groups: { [key: string]: T[] } = {};

	for (const item of array) {
		const key = callback(item);
		if (!groups[key]) groups[key] = [];
		groups[key]?.push(item);
	}

	return groups;
}
