<script lang="ts">
	import { createEventDispatcher, setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SegmentContext, SegmentItem } from './segment';

	export let wide = false;
	export let selectedIndex = 0;
	export let selected: string | undefined = undefined;

	let dispatch = createEventDispatcher<{ select: string }>();

	let indexesIterator = -1;
	let segments: SegmentItem[] = [];

	let focusedSegmentIndex = writable(-1);
	let selectedSegmentIndex = writable(selectedIndex);
	let length = writable(0);

	const context: SegmentContext = {
		focusedSegmentIndex,
		selectedSegmentIndex,
		length,
		setIndex: () => {
			indexesIterator += 1;
			return indexesIterator;
		},
		addSegment: ({ id, index, disabled }) => {
			segments = [...segments, { id, index, disabled }];
			length.set(segments.length);
			if (index == selectedIndex) selected = id;
		},
		setSelected: (segmentIndex) => {
			if (segmentIndex >= 0 && segmentIndex < segments.length) {
				$focusedSegmentIndex = segmentIndex;

				if (!segments[segmentIndex].disabled) {
					$selectedSegmentIndex = $focusedSegmentIndex;
					selected = segments[segmentIndex].id;
					dispatch('select', selected);
				}
			}
		}
	};
	setContext<SegmentContext>('SegmentedControl', context);
</script>

<div class="wrapper" class:wide>
	<slot />
</div>

<style lang="postcss">
	.wrapper {
		display: inline-flex;
		&.wide {
			display: flex;
		}
	}
</style>
