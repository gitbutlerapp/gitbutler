<script lang="ts">
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SegmentContext } from '$components/segmentControl/segmentTypes';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		selected?: string;
		fullWidth?: boolean;
		shrinkable?: boolean;
		size?: 'default' | 'small';
		onselect?: (id: string) => void;
		children: Snippet;
	}

	const {
		selected,
		fullWidth = false,
		shrinkable = false,
		size = 'default',
		onselect,
		children
	}: SegmentProps = $props();

	const registeredSegments: string[] = [];
	const selectedSegmentId = writable<string | undefined>(selected);

	// Sync external selected prop to internal store
	$effect(() => {
		if (selected !== undefined) {
			selectedSegmentId.set(selected);
		}
	});

	const context: SegmentContext = {
		selectedSegmentId,
		registerSegment: (id: string) => {
			if (!registeredSegments.includes(id)) {
				registeredSegments.push(id);
				// If no segment is selected, select the first one (silent initialization, do not call onselect)
				if ($selectedSegmentId === undefined) {
					selectedSegmentId.set(id);
					// Do not call onselect here to avoid side effects before user interaction
				}
			}
		},
		selectSegment: (id: string) => {
			selectedSegmentId.set(id);
			onselect?.(id);
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
