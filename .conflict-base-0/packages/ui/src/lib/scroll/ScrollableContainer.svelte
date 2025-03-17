<script lang="ts">
	import Scrollbar, { type ScrollbarPaddingType } from './Scrollbar.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		height?: string;
		maxHeight?: string;
		initiallyVisible?: boolean;
		wide?: boolean;
		padding?: ScrollbarPaddingType;
		shift?: string;
		thickness?: string;
		horz?: boolean;
		whenToShow: 'hover' | 'always' | 'scroll';
		onthumbdrag?: (dragging: boolean) => void;
		children: Snippet;
		onscrollEnd?: (visible: boolean) => void;
		onscroll?: (e: Event) => void;
	}

	const {
		height,
		maxHeight,
		initiallyVisible,
		wide,
		padding,
		shift,
		thickness,
		horz,
		whenToShow,
		children,
		onthumbdrag,
		onscroll,
		onscrollEnd
	}: Props = $props();

	let viewport = $state<HTMLDivElement>();
	let scrollEndVisible = $state<boolean>(false);

	$effect(() => {
		if (scrollEndVisible) {
			onscrollEnd?.(true);
		} else {
			onscrollEnd?.(false);
		}
	});
</script>

<div class="scrollable" style:flex-grow={wide ? 1 : 0} style:max-height={maxHeight}>
	<div
		bind:this={viewport}
		class="viewport hide-native-scrollbar"
		style:height
		style:overflow-y="auto"
		onscroll={(e) => {
			const target = e.target as HTMLDivElement;
			scrollEndVisible = target.scrollTop + target.clientHeight >= target.scrollHeight;

			onscroll?.(e);
		}}
	>
		{@render children()}
		<Scrollbar
			{whenToShow}
			{viewport}
			{initiallyVisible}
			{padding}
			{shift}
			{thickness}
			{horz}
			{onthumbdrag}
		/>
	</div>
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		flex-direction: column;
		position: relative;
		overflow: hidden;
		height: 100%;
	}
	.viewport {
		height: 100%;
		width: 100%;
	}
</style>
