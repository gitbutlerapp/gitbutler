import Component from '$components/test/change-during-async-sees-updates-two-components/Component.svelte';
import { ExternallyResolvedPromise } from '$lib/utils/resolveExternally';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { expect, test } from 'vitest';

/**
 * This shows the behavior of a variable changing while a promise is being
 * awaited. This time the value is passed through the child component.
 *
 * // Called by clicking "log"
 * async function logfn() {
 *     // Value should first be "hello"
 *     log(value);
 *     // The value prop is changed to "world"
 *     // The promise is then resolved
 *     await promise.promise;
 *     // The value that is logged is then "world"
 *     log(value);
 * }
 */
test('Component', async () => {
	const logs: string[] = [];
	// eslint-disable-next-line func-style
	const log = (value: string) => logs.push(value);
	const promise = new ExternallyResolvedPromise<undefined>();

	const user = userEvent.setup();
	render(Component, {
		props: {
			log,
			promise
		}
	});
	const logButton = await screen.findByText('log');
	const updateStateButton = await screen.findByText('update-state');

	await user.click(logButton);
	expect(logs).toEqual(['hello']);
	await user.click(updateStateButton);
	promise.resolve();
	await promise.promise;
	expect(logs).toEqual(['hello', 'world']);
});
