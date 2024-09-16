type ItemsSatisfyResult = 'all' | 'some' | 'none';

export function itemsSatisfy<T>(arr: T[], predicate: (item: T) => boolean): ItemsSatisfyResult {
	let satisfyCount = 0;
	let offenseCount = 0;
	for (const item of arr) {
		if (predicate(item)) {
			satisfyCount++;
			continue;
		}

		offenseCount++;
	}

	if (satisfyCount === 0) {
		return 'none';
	}

	if (offenseCount === 0) {
		return 'all';
	}

	return 'some';
}

export function chunk<T>(arr: T[], size: number) {
	return Array.from({ length: Math.ceil(arr.length / size) }, (_v, i) =>
		arr.slice(i * size, i * size + size)
	);
}

export function unique<T>(arr: T[]): T[] {
	return Array.from(new Set(arr));
}
