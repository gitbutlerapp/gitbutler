<script lang="ts">
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SegmentContext, SegmentItem } from './segmentTypes';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		selectedIndex: number;
		children: Snippet;
		onselect?: (id: string) => void;
	}

	const { selectedIndex, children, onselect }: SegmentProps = $props();

	let indexesIterator = -1;
	let segments: SegmentItem[] = [];

	let selectedSegmentIndex = writable(selectedIndex);

	const context: SegmentContext = {
		selectedSegmentIndex,
		setIndex: () => {
			indexesIterator += 1;
			return indexesIterator;
		},
		addSegment: ({ index }) => {
			segments = [...segments, { index }];
		},
		setSelected: ({ index: segmentIndex, id }) => {
			if (segmentIndex >= 0 && segmentIndex < segments.length) {
				$selectedSegmentIndex = segmentIndex;
				onselect && onselect(id);
			}
		}
	};

	setContext<SegmentContext>('SegmentControl', context);
</script>

<div class="wrapper">
	{@render children()}
</div>

<style lang="postcss">
	.wrapper {
		display: inline-flex;
	}
</style>
