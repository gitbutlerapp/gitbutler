export type Bullet = {
	prefix: string;
	indent: string;
	number?: number;
};
type WrapArgs = {
	line: string;
	maxLength: number;
	remainder?: string;
	indent?: string;
	bullet?: Bullet;
};

/**
 * Helper function for (re)wrapping lines. Takes a line, and gives back a line
 * that fits within `maxLength`, and the remainder that should be carried over
 * to the next line.
 */
export function wrapLine({ line, maxLength, remainder = '', indent = '', bullet }: WrapArgs): {
	newLine: string;
	newRemainder: string;
} {
	const parts = Array.from(line.substring(indent.length).match(/([ \t]+|\S+)/g) || []);
	let acc = remainder.length > 0 ? indent + remainder + ' ' : bullet ? bullet.prefix : indent;
	for (const word of parts) {
		if (acc.length + word.length > maxLength) {
			const newLine = acc ? acc : word;
			const concatLine = remainder
				? indent + remainder + ' ' + line.substring(indent.length)
				: line;
			const newRemainder = concatLine.slice(newLine.length).trim();

			return {
				newLine,
				newRemainder
			};
		}
		acc += word;
	}

	return { newLine: acc, newRemainder: '' };
}

type WrappingExemption = { regex: RegExp; name: string };

export const wrappingExemptions: WrappingExemption[] = [
	{ name: 'fenced code block', regex: /^ {0,3}```.*$|^ {0,3}~~~.*$/ },
	{ name: 'html block', regex: /^[ \t]*<([a-zA-Z]+)(\s[^>]*)?>/ },
	{ name: 'table row', regex: /^\|/ },
	{ name: 'block quote', regex: /^ *>/ },
	{ name: 'heading', regex: /^ {0,3}#{1,6} / },
	{ name: 'horizontal rule', regex: /^ {0,3}(-{3,}|\*{3,}|_{3,})\s*$/ },
	{ name: 'linked definition', regex: /^\s*\[[^\]]+]:\s+\S+/ },
	{ name: 'inline link or image', regex: /!?\[[^\]]*\]\([^)]*\)/ }
];

export function isWrappingExempt(line: string): WrappingExemption | undefined {
	return wrappingExemptions.find((exemption) => exemption.regex.test(line));
}

export function parseIndent(line: string) {
	return line.match(/^( +|\t+)/)?.[0] || '';
}

export function parseBullet(text: string): Bullet | undefined {
	const match = text.match(/^(\s*)([-*+]|(?<number>[0-9]+)\.)\s/);
	if (!match) return;
	const indent = match[1] ?? '';
	const prefix = match[0];
	const numberStr = match.groups?.['number'];
	const number = numberStr ? parseInt(numberStr) : undefined;
	return { prefix, indent: indent + '  ', number };
}
