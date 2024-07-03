// https://dmitripavlutin.com/how-to-compare-objects-in-javascript/
export function shallowEqual(object1: any | undefined, object2: any | undefined) {
	if (object1 === undefined || object2 === undefined) return;
	const keys1 = Object.keys(object1);
	const keys2 = Object.keys(object2);

	if (keys1.length !== keys2.length) {
		return false;
	}

	for (const key of keys1) {
		if (object1[key] !== object2[key]) {
			return false;
		}
	}

	return true;
}
