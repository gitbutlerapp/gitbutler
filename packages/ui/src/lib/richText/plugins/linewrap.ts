type WrapArgs = {
	line: string;
	maxLength: number;
	remainder?: string;
	indent?: string;
};

/**
 * Helper function for (re)wrapping lines. Takes a line, and gives back a line
 * that fits within `maxLength`, and the remainder that should be carried over
 * to the next line.
 */
export function wrapLine({ line, maxLength, remainder = '', indent = '' }: WrapArgs): {
	newLine: string;
	newRemainder: string;
} {
	const parts = Array.from(line.match(/([ \t]+|\S+)/g) || []);
	let acc = remainder.length > 0 ? remainder + ' ' : '';
	for (const word of parts) {
		if (indent.length + acc.length + word.length > maxLength) {
			const newLine = acc ? indent + acc : word;
			const concatLine = remainder ? remainder + ' ' + line : line;
			const newRemainder = concatLine.slice(newLine.length).trim();
			return {
				newLine,
				newRemainder
			};
		}
		acc += word;
	}

	return { newLine: indent + acc, newRemainder: '' };
}
