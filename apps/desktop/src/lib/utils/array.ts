type Predicate<T> = (item: T) => boolean;

type ItemsSatisfyResult = 'all' | 'some' | 'none';

export function itemsSatisfy<T>(arr: T[], predicate: Predicate<T>): ItemsSatisfyResult {
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

interface GroupByResult<T> {
	satisfied: T[];
	rest: T[];
}

export function groupByCondition<T>(arr: T[], predicate: Predicate<T>): GroupByResult<T> {
	const satisfied: T[] = [];
	const rest: T[] = [];

	for (const item of arr) {
		if (predicate(item)) {
			satisfied.push(item);
			continue;
		}

		rest.push(item);
	}

	return { satisfied, rest };
}
