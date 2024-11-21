<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import {
		decrement,
		increment,
		selectExampleValue,
		selectExampleValueGreaterThan
	} from '@gitbutler/shared/redux/example';
	import { AppDispatch, AppState } from '@gitbutler/shared/redux/store';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	/**
	 * A demo page for Redux. This can be accessed by typing
	 * `location = '/reduxExample'` in the console.
	 */

	const appState = getContext(AppState);
	const appDispatch = getContext(AppDispatch);

	let comparisonTarget = $state(4);

	const exampleState = appState.example;
	const currentValue = $derived(selectExampleValue($exampleState));
	const greaterThanComparisonTarget = $derived(
		selectExampleValueGreaterThan($exampleState, comparisonTarget)
	);
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
