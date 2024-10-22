export function camelCaseToTitleCase(input: string) {
	let result = input.charAt(0).toUpperCase();
	for (let i = 1; i < input.length; i++) {
		if (
			input.charAt(i) === input.charAt(i).toUpperCase() &&
			input.charAt(i) !== input.charAt(i).toLowerCase()
		) {
			result += ' ' + input.charAt(i);
		} else {
			result += input.charAt(i);
		}
	}
	return result;
}
