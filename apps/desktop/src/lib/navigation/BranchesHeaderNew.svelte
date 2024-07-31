<script lang="ts">
	import TextBox from '$lib/shared/TextBox.svelte';
	import Segment from '@gitbutler/ui/SegmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/SegmentControl/SegmentControl.svelte';
	import Badge from '@gitbutler/ui/shared/Badge.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		filteredBranchCount?: number;
		totalBranchCount: number;
		filterButton?: Snippet<[filtersActive: boolean]>;
		onSearch: (value: string) => void;
	}

	const { filteredBranchCount, onSearch }: Props = $props();

	let searchValueState = $state('');
</script>

<div class="header">
	<div class="branches-title">
		<span class="text-base-14 text-bold">Branches</span>

		{#if filteredBranchCount !== undefined}
			<Badge count={filteredBranchCount} />
		{/if}
	</div>

	<TextBox
		icon="search"
		placeholder="Search"
		on:input={(e) => onSearch(e.detail)}
		value={searchValueState}
	/>

	<SegmentControl fullWidth selectedIndex={0}>
		<Segment id="all">All</Segment>
		<Segment id="mine">PRs</Segment>
		<Segment id="active">Mine</Segment>
	</SegmentControl>
</div>

<style lang="postcss">
	.header {
		display: flex;
		flex-direction: column;
		color: var(--clr-scale-ntrl-0);
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 14px 14px 12px 14px;
		gap: 8px;
		border-bottom: 1px solid transparent;
		transition: border-bottom var(--transition-fast);
		position: relative;
	}
	.branches-title {
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>
