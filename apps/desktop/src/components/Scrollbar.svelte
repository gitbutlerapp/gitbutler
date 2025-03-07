<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import Scrollbar, { type ScrollbarPaddingType } from '@gitbutler/ui/scroll/Scrollbar.svelte';

	interface Props {
		viewport: Element;
		contents: Element;
		initiallyVisible?: boolean;
		thickness?: string;
		padding?: ScrollbarPaddingType;
		shift?: string;
		horz?: boolean;
		zIndex?: string;
		onthumbdrag?: (dragging: boolean) => void;
		onscroll?: (e: Event) => void;
	}

	const {
		viewport,
		contents,
		initiallyVisible = false,
		thickness = '0.563rem',
		padding = {},
		shift = '0',
		horz = false,
		zIndex = 'var(--z-lifted)',
		onthumbdrag,
		onscroll
	}: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
</script>

<Scrollbar
	{viewport}
	{contents}
	{initiallyVisible}
	{thickness}
	{padding}
	{shift}
	{horz}
	{zIndex}
	{onthumbdrag}
	{onscroll}
	whenToShow={$userSettings.scrollbarVisibilityState}
/>
