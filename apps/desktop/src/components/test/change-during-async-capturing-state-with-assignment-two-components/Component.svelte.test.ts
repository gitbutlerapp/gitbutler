import Component from "$components/test/change-during-async-capturing-state-with-assignment-two-components/Component.svelte";
import { ExternallyResolvedPromise } from "$lib/utils/resolveExternally";
import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";

/**
 * This shows the behavior of a "dereferencing" a state variable such that it is
 * no longer reactive. This variant reads from the props.
 *
 * // Called by clicking "log"
 * async function logfn() {
 *     const value2 = value;
 *     // Value should first be "hello"
 *     log(value2);
 *     // The state variable is changed to "world"
 *     // The promise is then resolved
 *     await promise.promise;
 *     // The value that is logged is then "world"
 *     log(value);
 *     // The "dereferenced" value is still "hello"
 *     log(value2);
 * }
 */
test("Component", async () => {
	const logs: string[] = [];
	// eslint-disable-next-line func-style
	const log = (value: string) => logs.push(value);
	const promise = new ExternallyResolvedPromise<undefined>();

	const user = userEvent.setup();
	render(Component, {
		props: {
			log,
			promise,
		},
	});
	const logButton = await screen.findByText("log");
	const updateStateButton = await screen.findByText("update-state");

	await user.click(logButton);
	expect(logs).toEqual(["hello"]);
	await user.click(updateStateButton);
	promise.resolve();
	await promise.promise;
	expect(logs).toEqual(["hello", "world"]);
});
