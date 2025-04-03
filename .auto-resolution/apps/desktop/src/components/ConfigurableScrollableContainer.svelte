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
		onthumbdrag?: (dragging: boolean) => void;
		children: Snippet;
		onscrollTop?: (visible: boolean) => void;
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
		children,
		onthumbdrag,
		onscroll,
		onscrollTop,
		onscrollEnd
	}: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
</script>

<ScrollableContainer
	{height}
	{maxHeight}
	{initiallyVisible}
	{wide}
	{padding}
	{shift}
	{thickness}
	{horz}
	{onthumbdrag}
	{onscrollTop}
	{onscrollEnd}
	{onscroll}
	whenToShow={$userSettings.scrollbarVisibilityState}
>
	{@render children()}
</ScrollableContainer>
