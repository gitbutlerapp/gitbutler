<script lang="ts">
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SegmentContext, SegmentItem } from '$lib/segmentControl/segmentTypes';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		defaultIndex: number;
		fullWidth?: boolean;
		shrinkable?: boolean;
		size?: 'default' | 'small';
		onselect?: (id: string) => void;
		children: Snippet;
	}

	const {
		defaultIndex,
		fullWidth,
		shrinkable = false,
		size,
		onselect,
		children
	}: SegmentProps = $props();

	let indexesIterator = -1;
	let segments: SegmentItem[] = [];

	let selectedSegmentIndex = writable(defaultIndex);

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
				if (onselect) {
					onselect(id);
				}
			}
		}
	};

	setContext<SegmentContext>('SegmentControl', context);
</script>

<div
	class="segment-control-container"
	class:shrinkable
	class:small={size === 'small'}
	class:full-width={fullWidth}
>
	{@render children()}
</div>
