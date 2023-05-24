export type Bucket = [number, number];

export const generateBuckets = (timestamps: number[], bucketCount: number): Bucket[] => {
	// 1. Find the minimum and maximum timestamps
	const min = Math.min(...timestamps);
	const max = Math.max(...timestamps);

	// 2. Calculate the range and bucket size
	const range = max - min;
	const bucketSize = range / bucketCount;

	// 3. Create an empty array of buckets
	const buckets: Bucket[] = [];
	for (let i = 0; i < bucketCount; i++) {
		const from = min + i * bucketSize;
		const to = min + (i + 1) * bucketSize;
		buckets.push([from, to]);
	}
	return buckets;
};

export const fillBuckets = (timestamps: number[], buckets: Bucket[]): number[][] => {
	const groups: number[][] = new Array(buckets.length).fill(null).map(() => []);
	for (const timestamp of timestamps) {
		const index = buckets.findIndex(([min, max]) => timestamp >= min && timestamp <= max);
		groups[index].push(timestamp);
	}
	return groups;
};
