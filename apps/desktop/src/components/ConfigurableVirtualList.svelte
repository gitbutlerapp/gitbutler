<script lang="ts" module>
	type T = unknown;
</script>

<script lang="ts" generics="T">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import { VirtualList } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	type Props = {
		items: Array<T>;
		children?: Snippet<[]>;
		chunkTemplate: Snippet<[T[]]>;
		batchSize: number;
		onloadmore?: () => Promise<void>;
		grow?: boolean;
		stickToBottom?: boolean;
		defaultHeight: number;
		padding?: {
			left?: number;
			right?: number;
			top?: number;
			bottom?: number;
		};
	};

	const {
		items,
		children,
		chunkTemplate,
		batchSize,
		onloadmore,
		grow,
		padding,
		defaultHeight,
		stickToBottom = false
	}: Props = $props();

	const userSettings = inject(SETTINGS);
	let virtualList: VirtualList<T>;

	// Export method to scroll to bottom
	export function scrollToBottom() {
		if (virtualList?.scrollToBottom) {
			virtualList.scrollToBottom();
		}
	}
</script>

<VirtualList
	bind:this={virtualList}
	{items}
	{batchSize}
	{defaultHeight}
	visibility={$userSettings.scrollbarVisibilityState}
	{grow}
	{stickToBottom}
	{onloadmore}
	{padding}
	{chunkTemplate}
	{children}
/>
