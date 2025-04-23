import { deduplicateBy } from '@gitbutler/shared/utils/array';

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

export function unique<T>(arr: T[]): T[] {
	return Array.from(new Set(arr));
}

export function uniqueBy<T, K extends string>(arr: T[], keyFn: (item: T) => K): T[] {
	type Result = { item: T; key: K };
	const results: Result[] = arr.map((item) => ({
		item,
		key: keyFn(item)
	}));

	return deduplicateBy(results, 'key').map((r) => r.item);
}

export function outerJoinBy<T, K extends string>(arrA: T[], arrB: T[], keyFn: (item: T) => K): T[] {
	const arrMap = new Map<K, T>();

	for (const item of arrA) {
		const key = keyFn(item);
		arrMap.set(key, item);
	}

	for (const item of arrB) {
		const key = keyFn(item);
		if (arrMap.has(key)) {
			arrMap.delete(key);
			continue;
		}
		arrMap.set(key, item);
	}

	return Array.from(arrMap.values());
}

export function leftJoinBy<T, K extends string>(arrA: T[], arrB: T[], keyFn: (item: T) => K): T[] {
	const arrMap = new Map<K, T>();

	for (const item of arrA) {
		const key = keyFn(item);
		arrMap.set(key, item);
	}

	for (const item of arrB) {
		const key = keyFn(item);
		if (arrMap.has(key)) {
			arrMap.delete(key);
			continue;
		}
	}

	return Array.from(arrMap.values());
}
