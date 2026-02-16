import { unsubscribe } from '$lib/utils/unsubscribe';
import { expect, test, describe } from 'vitest';

describe.concurrent('unsubscribe', () => {
	test('When provided an undefined value, it resolves to undefined', async () => {
		expect(await unsubscribe(undefined)()).toEqual([undefined]);
	});

	test('When provided a function, it resolves to that functions return value', async () => {
		// eslint-disable-next-line func-style
		const subscription = () => 42;
		expect(await unsubscribe(subscription)()).toEqual([42]);
	});

	test('When provided a promise of a function, it resolves to that functions return value', async () => {
		const subscription = Promise.resolve(() => 42);
		expect(await unsubscribe(subscription)()).toEqual([42]);
	});

	test('When provided all three different combinations, each one resolves to the final value', async () => {
		const subscription1 = undefined;
		// eslint-disable-next-line func-style
		const subscription2 = () => 42;
		const subscription3 = Promise.resolve(() => 420);

		expect(await unsubscribe(subscription1, subscription2, subscription3)()).toEqual([
			undefined,
			42,
			420
		]);
	});
});
