import { wrapLine } from '$lib/richText/linewrap';
import { expect, test, describe } from 'vitest';

describe('wrapline', () => {
	test('simple', () => {
		const { newLine, newRemainder: remainder } = wrapLine({
			line: 'hello world',
			maxLength: 8
		});
		expect(newLine).toEqual('hello ');
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
		expect(newLine).toEqual(' ');
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
		expect(newLine).toEqual('longer ');
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
		expect(newLine).toEqual(' leading ');
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
		expect(newLine).toEqual('- hello ');
		expect(remainder).toEqual('world');
	});
});
