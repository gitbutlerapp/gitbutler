<!--
@component
A three way split view that manages resizing of the panels.

The left and right hand side are set in rem units, leaving the middle to grow
as the window resizes. If the window shrinks to where it is smaller than the
sum of the preferred widths, then the derived widths adjust down, with the left
hand side shrinking before the right hand side.

Persisted widths are only stored when resizing manually, meaning you can shrink
the window, then enlarge it and retain the original widths of the layout.

@example
```
<MainViewport
	name="workspace"
	leftWidth={{ default: 200, min: 100}}
	rightWidth={{ default: 200, min: 100}}
>
	{#snippet left()} {/snippet}
	{#snippet middle()} {/snippet}
	{#snippet right()} {/snippet}
</MainViewport>
```
-->
<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import type { Snippet } from 'svelte';

	type Props = {
		name: string;
		left: Snippet;
		middle: Snippet;
		right: Snippet;
		leftWidth: {
			default: number;
			min: number;
		};
		rightWidth: {
			default: number;
			min: number;
		};
	};

	const { name, left, middle, right, leftWidth, rightWidth }: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	let leftPreferredWidth = $derived(
		persisted(pxToRem(leftWidth.default, zoom), `$main_view_left_${name}`)
	);
	let rightPreferredWidth = $derived(
		persisted(pxToRem(rightWidth.default, zoom), `$main_view_right_${name}`)
	);

	let leftDiv = $state<HTMLDivElement>();
	let middleDiv = $state<HTMLDivElement>();
	let rightDiv = $state<HTMLDivElement>();

	const leftMinWidth = $derived(pxToRem(leftWidth.min, zoom));
	const rightMinWidth = $derived(pxToRem(rightWidth.min, zoom));

	// These need to stay in px since they are bound to elements.
	let containerBindWidth = $state<number>(1000); // TODO: What initial value should we give this?
	let middleBindWidth = $state<number>(540);
	let leftBindWidth = $state<number>(300);
	let rightBindWidth = $state<number>(300);

	// When true the right hand side will resize to accommodate changes to
	// the width of the left hand side.
	let reverse = $state(false);

	// Total width we cannot go below.
	const padding = $derived(containerBindWidth - window.innerWidth);
	const containerMinWidth = $derived(804 - padding);

	const middleMinWidth = $derived(pxToRem(containerMinWidth, zoom) - leftMinWidth - rightMinWidth);

	// Left side max width depends on teh size of the right side, unless
	// `reverse` is true.
	const leftMaxWidth = $derived(
		pxToRem(containerBindWidth, zoom) -
			middleMinWidth -
			(reverse ? rightMinWidth : pxToRem(rightBindWidth, zoom)) -
			1 // From the flex box gaps
	);

	// Right side has priority over the left (unless `reverse` is true), so
	// its max size is the theoretical max.
	const rightMaxWidth = $derived(
		pxToRem(containerBindWidth, zoom) -
			middleMinWidth -
			(reverse ? pxToRem(leftBindWidth, zoom) : leftMinWidth) -
			1 // From the flex box gaps
	);

	const derivedLeftWidth = $derived(
		Math.min(leftMaxWidth, Math.max(leftMinWidth, $leftPreferredWidth))
	);
	const derivedRightWidth = $derived(
		Math.min(rightMaxWidth, Math.max(rightMinWidth, $rightPreferredWidth))
	);
</script>

<div
	class="main-viewport"
	use:focusable={{ id: DefinedFocusable.MainViewport }}
	bind:clientWidth={containerBindWidth}
>
	<div
		class="left"
		bind:this={leftDiv}
		bind:clientWidth={leftBindWidth}
		style:width={derivedLeftWidth + 'rem'}
		style:min-width={leftMinWidth + 'rem'}
		use:focusable={{ id: DefinedFocusable.ViewportLeft, parentId: DefinedFocusable.MainViewport }}
	>
		{@render left()}
		<Resizer
			viewport={leftDiv}
			direction="right"
			minWidth={leftMinWidth}
			maxWidth={leftMaxWidth}
			borderRadius="ml"
			onWidth={(value) => {
				leftPreferredWidth.set(value);
				rightPreferredWidth.set(pxToRem(rightBindWidth, zoom));
			}}
			onResizing={(isResizing) => {
				if (isResizing) {
					// Before flipping the reverse bool we need to set the
					// preferred width to the actual width, to prevent content
					// from shifting.
					leftPreferredWidth.set(pxToRem(leftBindWidth, zoom));
				}
				reverse = isResizing;
			}}
		/>
	</div>
	<div
		class="middle"
		bind:this={middleDiv}
		bind:clientWidth={middleBindWidth}
		style:min-width={middleMinWidth + 'rem'}
		use:focusable={{ id: DefinedFocusable.ViewportMiddle, parentId: DefinedFocusable.MainViewport }}
	>
		{@render middle()}
	</div>
	<div
		class="right"
		bind:this={rightDiv}
		bind:clientWidth={rightBindWidth}
		style:width={derivedRightWidth + 'rem'}
		style:min-width={rightMinWidth + 'rem'}
		use:focusable={{ id: DefinedFocusable.ViewportRight, parentId: DefinedFocusable.MainViewport }}
	>
		{@render right()}
		<Resizer
			viewport={rightDiv}
			direction="left"
			minWidth={rightMinWidth}
			maxWidth={rightMaxWidth}
			borderRadius="ml"
			onWidth={(value) => {
				rightPreferredWidth.set(value);
				leftPreferredWidth.set(pxToRem(leftBindWidth, zoom));
			}}
		/>
	</div>
</div>

<style lang="postcss">
	.main-viewport {
		display: flex;
		position: relative;
		flex: 1;
		align-items: stretch;
		width: 100%;
		height: 100%;
		overflow: auto;
		gap: 8px;
	}

	.left {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
	}

	.middle {
		display: flex;
		position: relative;
		flex-grow: 1;
		flex-shrink: 1;
		flex-direction: column;
		overflow-x: hidden;
		gap: 8px;
		border-radius: var(--radius-ml);
	}

	.right {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
		overflow: hidden;
	}
</style>
