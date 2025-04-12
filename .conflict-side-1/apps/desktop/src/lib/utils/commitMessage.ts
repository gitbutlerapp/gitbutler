export function splitMessage(message: string) {
	const splitIndex = message.indexOf('\n');
	const title = splitIndex !== -1 ? message.substring(0, splitIndex) : message;
	const description = splitIndex !== -1 ? message.substring(splitIndex + 1) : '';

	return {
		title: title.trim(),
		description: description.trim()
	};
}
