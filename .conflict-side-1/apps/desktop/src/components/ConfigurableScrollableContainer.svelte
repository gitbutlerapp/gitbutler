<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import ScrollableContainer from '@gitbutler/ui/scroll/ScrollableContainer.svelte';
	import { type ScrollbarPaddingType } from '@gitbutler/ui/scroll/Scrollbar.svelte';
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
		autoScroll?: boolean;
		zIndex?: string;
		onthumbdrag?: (dragging: boolean) => void;
		children: Snippet;
		onscrollTop?: (visible: boolean) => void;
		onscrollEnd?: (visible: boolean) => void;
		onscroll?: (e: Event) => void;
		onscrollexists?: (exists: boolean) => void;
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
		autoScroll,
		zIndex,
		children,
		onthumbdrag,
		onscroll,
		onscrollTop,
		onscrollEnd,
		onscrollexists
	}: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	let scroller: ScrollableContainer;
</script>

<ScrollableContainer
	bind:this={scroller}
	{height}
	{maxHeight}
	{initiallyVisible}
	{wide}
	{padding}
	{shift}
	{thickness}
	{horz}
	{autoScroll}
	{zIndex}
	{onthumbdrag}
	{onscrollTop}
	{onscrollEnd}
	{onscroll}
	{onscrollexists}
	whenToShow={$userSettings.scrollbarVisibilityState}
>
	{@render children()}
</ScrollableContainer>
