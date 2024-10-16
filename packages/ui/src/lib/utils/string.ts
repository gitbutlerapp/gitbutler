export function camelCaseToTitleCase(input: string) {
	return input
		.split(/(?<![A-Z])(?=[A-Z])/)
		.map(function (word: string) {
			return word.charAt(0).toUpperCase() + word.slice(1);
		})
		.join(' ');
}
