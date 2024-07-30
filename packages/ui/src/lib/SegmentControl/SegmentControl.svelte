<script lang="ts">
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SegmentContext, SegmentItem } from './segmentTypes';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		selectedIndex: number;
		fullWidth?: boolean;
		onselect?: (id: string) => void;
		children: Snippet;
	}

	const { selectedIndex, fullWidth, onselect, children }: SegmentProps = $props();

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

<div class="wrapper" class:full-width={fullWidth}>
	{@render children()}
</div>

<style lang="postcss">
	.wrapper {
		display: inline-flex;
	}

	.wrapper.full-width {
		width: 100%;
	}
</style>
