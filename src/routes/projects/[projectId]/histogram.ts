type Timestamp = number;

export const bucketByTimestamp = (timestamps: Timestamp[], bucketCount: number): Timestamp[][] => {
	// 1. Find the minimum and maximum timestamps
	const min = Math.min(...timestamps);
	const max = Math.max(...timestamps);

	// 2. Calculate the range and bucket size
	const range = max - min;
	const bucketSize = range / bucketCount;

	// 3. Create an empty array of buckets
	const buckets: Timestamp[][] = new Array(bucketCount).fill(null).map(() => []);

	// 4. Iterate through the timestamps, find the corresponding bucket, and push the timestamp into the bucket
	for (const timestamp of timestamps) {
		let bucketIndex = Math.floor((timestamp - min) / bucketSize);
		if (bucketIndex === bucketCount) {
			bucketIndex--; // Edge case: if the timestamp is equal to the max, assign it to the last bucket
		}
		if (!bucketIndex) {
			bucketIndex = bucketCount - 1;
		}
		buckets[bucketIndex].push(timestamp);
	}
	return buckets;
};
