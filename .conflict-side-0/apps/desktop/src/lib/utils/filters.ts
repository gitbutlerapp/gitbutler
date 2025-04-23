// If a value occurs > 1 times then all but one will fail this condition.
export function unique(value: any, index: number, array: any[]) {
	return array.indexOf(value) === index;
}

export function uniqeByPropValues<T extends object>(value: T, index: number, array: T[]): boolean {
	if (value === null) {
		return false;
	}

	const propertyKeys = Object.keys(value) as (keyof T)[];
	return array.findIndex((v) => propertyKeys.every((key) => v?.[key] === value[key])) === index;
}
