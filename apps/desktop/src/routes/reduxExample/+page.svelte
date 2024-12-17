<script lang="ts">
	import {
		desktopDecrement,
		DesktopDispatch,
		desktopIncrement,
		DesktopState
	} from '$lib/redux/store.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import {
		decrement,
		increment,
		selectExampleValue,
		selectExampleValueGreaterThan
	} from '@gitbutler/shared/redux/example';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	/**
	 * A demo page for Redux. This can be accessed by typing
	 * `location = '/reduxExample'` in the console.
	 */

	const appState = getContext(DesktopState);
	const appDispatch = getContext(DesktopDispatch);

	let comparisonTarget = $state(4);

	const currentValue = $derived(selectExampleValue(appState.example));
	const greaterThanComparisonTarget = $derived(
		selectExampleValueGreaterThan(appState.example, comparisonTarget)
	);

	const currentDesktopValue = $derived(appState.desktopOnly.value);
</script>

<div class="example-container">
	<Button onclick={() => goto('/')} type="button">Go back</Button>

	<h1>Redux Example</h1>
	<p>Current value: {currentValue}</p>

	<div>
		<Button onclick={() => appDispatch.dispatch(increment())} type="button">increase</Button>
		<Button onclick={() => appDispatch.dispatch(decrement())} type="button">decrease</Button>
	</div>
	<hr />
	<p>
		Is current value greater than <input type="number" bind:value={comparisonTarget} />? {greaterThanComparisonTarget}
	</p>

	<hr />

	<h1>Redux Desktop Only Example</h1>
	<p>Current value: {currentDesktopValue}</p>

	<div>
		<Button onclick={() => appDispatch.dispatch(desktopIncrement())} type="button">increase</Button>
		<Button onclick={() => appDispatch.dispatch(desktopDecrement())} type="button">decrease</Button>
	</div>
	<hr />
</div>

<style lang="postcss">
	.example-container {
		margin: auto;

		display: flex;
		flex-direction: column;
		align-items: center;

		gap: 8px;
	}
</style>
