// If a value occurs > 1 times then all but one will fail this condition.
export function unique(value: any, index: number, array: any[]) {
	return array.indexOf(value) === index;
}
