const commitMessageSplitRegExp = /(.*?)(?:\n+|$)(.*)/s;

export function splitMessage(message: string) {
	const matches = message.match(commitMessageSplitRegExp);

	return {
		title: matches?.[1] || '',
		description: matches?.[2] || ''
	};
}
