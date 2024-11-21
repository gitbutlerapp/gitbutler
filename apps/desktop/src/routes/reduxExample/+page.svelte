<script lang="ts">
	import {
		decrement,
		increment,
		selectExampleValue,
		selectExampleValueGreaterThan
	} from '@gitbutler/shared/redux/example';
	import { useDispatch, useStore } from '@gitbutler/shared/redux/utils';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	/**
	 * A demo page for Redux. This can be accessed by typing
	 * `location = '/reduxExample'` in the console.
	 */

	const store = useStore();
	const dispatch = useDispatch();

	let comparisonTarget = $state(4);

	const currentValue = $derived(selectExampleValue($store));
	const greaterThanComparisonTarget = $derived(
		selectExampleValueGreaterThan($store, comparisonTarget)
	);
</script>

<div class="example-container">
	<Button onclick={() => goto('/')} type="button">Go back</Button>

	<h1>Redux Example</h1>
	<p>Current value: {currentValue}</p>

	<div>
		<Button onclick={() => dispatch(increment())} type="button">increase</Button>
		<Button onclick={() => dispatch(decrement())} type="button">decrease</Button>
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
