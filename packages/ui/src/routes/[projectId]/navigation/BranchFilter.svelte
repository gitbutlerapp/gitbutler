<script lang="ts" context="module">
	export type TypeFilter = 'all' | 'branch' | 'pr';
</script>

<script lang="ts">
	import Segment from '$lib/components/SegmentControl/Segment.svelte';
	import SegmentedControl from '$lib/components/SegmentControl/SegmentedControl.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import type { BehaviorSubject } from 'rxjs';

	export let textFilter$: BehaviorSubject<string | undefined>;
	export let typeFilter$: BehaviorSubject<TypeFilter>;

	let options: { id: TypeFilter; name: string }[] = [
		{ id: 'all', name: 'All' },
		{ id: 'branch', name: 'Branch' },
		{ id: 'pr', name: 'Pull request' }
	];

	function onSelect(id: string) {
		typeFilter$.next(id as TypeFilter);
	}
</script>

<div class="wrapper">
	<TextBox icon="filter" on:input={(e) => textFilter$.next(e.detail)} />
	<div class="filter-btns">
		<SegmentedControl on:select={(e) => onSelect(e.detail)} wide selectedIndex={0}>
			{#each options as option}
				<Segment id={option.id} label={option.name} />
			{/each}
		</SegmentedControl>
	</div>
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
	}
	.filter-btns {
		width: 100%;
	}
</style>
