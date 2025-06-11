<!--
@component
A three way split view that manages resizing of the panels.

The left panel is set in rem units, the middle has fixed width constraints,
and the right panel grows as the window resizes. If the window shrinks to where 
it is smaller than the sum of the preferred widths, then the derived widths adjust 
down, with the left hand side shrinking before the middle panel.

Persisted widths are only stored when resizing manually, meaning you can shrink
the window, then enlarge it and retain the original widths of the layout.

@example
```
<MainViewport
	name="workspace"
	leftWidth={{ default: 200, min: 100}}
	middleWidth={{ default: 200, min: 100}}
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
		middleWidth: {
			default: number;
			min: number;
		};
	};

	const { name, left, middle, right, leftWidth, middleWidth }: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	let leftPreferredWidth = $derived(
		persisted(pxToRem(leftWidth.default, zoom), `$main_view_left_${name}`)
	);
	let middlePreferredWidth = $derived(
		persisted(pxToRem(middleWidth.default, zoom), `$main_view_middle_${name}`)
	);

	let leftDiv = $state<HTMLDivElement>();
	let middleDiv = $state<HTMLDivElement>();
	let rightDiv = $state<HTMLDivElement>();

	const leftMinWidth = $derived(pxToRem(leftWidth.min, zoom));
	const middleMinWidth = $derived(pxToRem(middleWidth.min, zoom));

	// These need to stay in px since they are bound to elements.
	let containerBindWidth = $state<number>(1000); // TODO: What initial value should we give this?
	let middleBindWidth = $state<number>(300);
	let leftBindWidth = $state<number>(300);
	let rightBindWidth = $state<number>(540);

	// Total width we cannot go below.
	const padding = $derived(containerBindWidth - window.innerWidth);
	const containerMinWidth = $derived(804 - padding);

	// When swapped, the middle becomes flexible and right becomes fixed
	const flexibleMinWidth = $derived(
		pxToRem(containerMinWidth, zoom) - leftMinWidth - middleMinWidth
	);
	const totalAvailableWidth = $derived(pxToRem(containerBindWidth, zoom) - flexibleMinWidth - 1); // Reserve space for flexible panel and gaps

	// Calculate derived widths with proper constraints
	const derivedLeftWidth = $derived(
		Math.min(totalAvailableWidth - middleMinWidth, Math.max(leftMinWidth, $leftPreferredWidth))
	);

	// Fixed panel width is constrained by remaining space after left panel
	const remainingForFixed = $derived(totalAvailableWidth - derivedLeftWidth);
	const derivedMiddleWidth = $derived(
		Math.min(remainingForFixed, Math.max(middleMinWidth, $middlePreferredWidth))
	);

	// Calculate max widths for the resizers
	const leftMaxWidth = $derived(totalAvailableWidth - middleMinWidth);
	const middleMaxWidth = $derived(totalAvailableWidth - derivedLeftWidth);
</script>

<div
	class="main-viewport"
	use:focusable={{ id: DefinedFocusable.MainViewport }}
	bind:clientWidth={containerBindWidth}
>
	<!-- Default layout: no swapping -->
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
			}}
		/>
	</div>

	<div
		class="middle fixed"
		bind:this={middleDiv}
		bind:clientWidth={middleBindWidth}
		style:width={derivedMiddleWidth + 'rem'}
		style:min-width={middleMinWidth + 'rem'}
		use:focusable={{
			id: DefinedFocusable.ViewportMiddle,
			parentId: DefinedFocusable.MainViewport
		}}
	>
		{@render middle()}
		<Resizer
			viewport={middleDiv}
			direction="right"
			minWidth={middleMinWidth}
			maxWidth={middleMaxWidth}
			borderRadius="ml"
			onWidth={(value) => {
				middlePreferredWidth.set(value);
			}}
		/>
	</div>

	<div
		class="right flexible"
		bind:this={rightDiv}
		bind:clientWidth={rightBindWidth}
		style:min-width={flexibleMinWidth + 'rem'}
		use:focusable={{
			id: DefinedFocusable.ViewportRight,
			parentId: DefinedFocusable.MainViewport
		}}
	>
		{@render right()}
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

	.middle.fixed {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
		overflow: hidden;
	}

	.right.flexible {
		display: flex;
		position: relative;
		flex-grow: 1;
		flex-shrink: 1;
		flex-direction: column;
		overflow-x: hidden;
	}
</style>
