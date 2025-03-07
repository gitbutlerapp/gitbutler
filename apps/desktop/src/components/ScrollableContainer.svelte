<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import ScrollableContainer from '@gitbutler/ui/scroll/ScrollableContainer.svelte';
	import { type ScrollbarPaddingType } from '@gitbutler/ui/scroll/Scrollbar.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		height?: string;
		fillViewport?: boolean;
		maxHeight?: string;
		initiallyVisible?: boolean;
		wide?: boolean;
		padding?: ScrollbarPaddingType;
		shift?: string;
		thickness?: string;
		horz?: boolean;
		onthumbdrag?: (dragging: boolean) => void;
		children: Snippet;
		onscrollEnd?: (visible: boolean) => void;
		onscroll?: (e: Event) => void;
	}

	const {
		height,
		fillViewport,
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
		onscrollEnd
	}: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
</script>

<ScrollableContainer
	{height}
	{fillViewport}
	{maxHeight}
	{initiallyVisible}
	{wide}
	{padding}
	{shift}
	{thickness}
	{horz}
	{onthumbdrag}
	{onscrollEnd}
	{onscroll}
	whenToShow={$userSettings.scrollbarVisibilityState}
>
	{@render children()}
</ScrollableContainer>
