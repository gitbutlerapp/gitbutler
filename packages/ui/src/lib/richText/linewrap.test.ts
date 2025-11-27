import { wrapLine } from '$lib/richText/linewrap';
import { expect, test, describe } from 'vitest';

describe('wrapline', () => {
	test('simple', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'hello world',
			maxLength: 8
		});
		expect(newLine).toEqual('hello');
		expect(remainder).toEqual('world');
	});

	test('whitespace exceeds', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'hello world ',
			maxLength: 11
		});
		expect(newLine).toEqual('hello world');
		expect(remainder).toEqual('');
	});

	test('trailing space', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'space ',
			maxLength: 5
		});
		expect(newLine).toEqual('space');
		expect(remainder).toEqual('');
	});

	test('whitepsace only', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: ' ',
			maxLength: 5
		});
		// Whitespace-only lines get trimmed to empty
		expect(newLine).toEqual('');
		expect(remainder).toEqual('');
	});

	test('nowrap', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'hello world',
			maxLength: 11
		});
		expect(newLine).toEqual('hello world');
		expect(remainder).toEqual('');
	});

	test('long remainder', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'short',
			remainder: 'longer',
			maxLength: 5
		});
		expect(newLine).toEqual('longer');
		expect(remainder).toEqual('short');
	});

	test('unbreakable word', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'unbreakable word',
			remainder: '',
			maxLength: 4
		});
		expect(newLine).toEqual('unbreakable');
		expect(remainder).toEqual('word');
	});

	test('leading space', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: ' leading space',
			remainder: '',
			maxLength: 10
		});
		expect(newLine).toEqual(' leading');
		expect(remainder).toEqual('space');
	});

	test('preserve spaces', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'we   test',
			remainder: '',
			maxLength: 9
		});
		expect(newLine).toEqual('we   test');
		expect(remainder).toEqual('');
	});

	test('bullet point', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: '- hello world',
			remainder: '',
			indent: '  ',
			bullet: { indent: '  ', prefix: '- ' },
			maxLength: 10
		});
		expect(newLine).toEqual('- hello');
		expect(remainder).toEqual('world');
	});

	test('commit message line under 72 chars should not wrap', () => {
		// Line is 68 chars, should not wrap
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'The introduction of InlineCodeNode revealed that the text editor was',
			maxLength: 72
		});
		expect(newLine).toEqual('The introduction of InlineCodeNode revealed that the text editor was');
		expect(remainder).toEqual('');
	});

	test('commit message second line under 72 chars should not wrap', () => {
		// Line is 68 chars, should not wrap
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'operating in a simplified rich text mode rather than plaintext mode,',
			maxLength: 72
		});
		expect(newLine).toEqual('operating in a simplified rich text mode rather than plaintext mode,');
		expect(remainder).toEqual('');
	});

	test('line that exceeds 72 chars by 1', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'since visually distinct backtick-quoted text requires rich text features.',
			maxLength: 72
		});
		// Line is 73 chars, should wrap
		expect(newLine.length).toBeLessThanOrEqual(72);
		expect(remainder).toBeTruthy();
		// Verify no characters are lost
		const reconstructed = newLine.trim() + ' ' + remainder;
		expect(reconstructed).toEqual(
			'since visually distinct backtick-quoted text requires rich text features.'
		);
	});

	test('bullet line that exceeds 72 chars', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: '- Incorrect rewrapping when editing in the middle of auto-wrapped paragraphs',
			maxLength: 72,
			bullet: { prefix: '- ', indent: '  ' }
		});
		// Line is 76 chars, should wrap
		expect(newLine.length).toBeLessThanOrEqual(72);
		expect(remainder).toBeTruthy();
		// Verify no characters are lost (accounting for bullet formatting)
		const originalText =
			'Incorrect rewrapping when editing in the middle of auto-wrapped paragraphs';
		const newText = newLine.substring(2).trim(); // Remove '- ' prefix
		const reconstructed = newText + ' ' + remainder;
		expect(reconstructed).toEqual(originalText);
	});
});
