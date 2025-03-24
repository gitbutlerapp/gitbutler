export function shallowCompare(left: unknown, right: unknown): boolean {
	if (left === right) {
		return true;
	}

	if (
		typeof left !== 'object' ||
		typeof right !== 'object' ||
		left === undefined ||
		right === undefined ||
		left === null ||
		right === null
	) {
		return false;
	}

	const keys1 = Object.keys(left);
	const keys2 = Object.keys(right);

	if (keys1.length !== keys2.length) {
		return false;
	}

	for (const key of keys1) {
		if ((left as Record<string, any>)[key] !== (right as Record<string, any>)[key]) {
			return false;
		}
	}

	return true;
}
