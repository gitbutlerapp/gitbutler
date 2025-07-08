import { wrapIfNecessary } from '$lib/richText/markdown';
import { expect, test, describe } from 'vitest';

describe('markdown', () => {
	/**
	 * Simple test that helped me get this roughly correct.
	 *
	 * TODO: Put more effort into the rich text <-> markdown transition.
	 */
	test('simple wrap', () => {
		const paragraph = 'hello world\n\nsecond';
		const wrapped = wrapIfNecessary(paragraph, 10);
		expect(wrapped).toEqual('hello \nworld\n\nsecond');
	});
});
