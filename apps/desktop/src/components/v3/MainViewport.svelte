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
	import AsyncRender from '$components/v3/AsyncRender.svelte';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import type { Snippet } from 'svelte';

	type Props = {
		name: string;
		left: Snippet;
		middle: Snippet;
		right: Snippet;
		drawerRight?: Snippet;
		leftWidth: {
			default: number;
			min: number;
		};
		middleWidth?: {
			default: number;
			min: number;
		};
		middleOpen?: boolean;
	};

	const { name, left, middle, right, drawerRight, leftWidth, middleWidth, middleOpen }: Props =
		$props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	let leftPreferredWidth = $derived(pxToRem(leftWidth.default, zoom));
	let middlePreferredWidth = $derived(pxToRem(middleWidth?.default, zoom));
	let rightDrawerPreferredWidth = $derived(pxToRem(24, zoom));

	let leftDiv = $state<HTMLDivElement>();
	let middleDiv = $state<HTMLDivElement>();
	let rightDiv = $state<HTMLDivElement>();
	let drawerRightDiv = $state<HTMLDivElement>();

	const leftMinWidth = $derived(pxToRem(leftWidth.min, zoom));
	const middleMinWidth = $derived(pxToRem(middleWidth?.min, zoom));

	// These need to stay in px since they are bound to elements.
	let containerBindWidth = $state<number>(1000); // TODO: What initial value should we give this?

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
		Math.min(totalAvailableWidth - middleMinWidth, Math.max(leftMinWidth, leftPreferredWidth))
	);

	// Fixed panel width is constrained by remaining space after left panel
	const remainingForFixed = $derived(totalAvailableWidth - derivedLeftWidth);
	const derivedMiddleWidth = $derived(
		Math.min(remainingForFixed, Math.max(middleMinWidth, middlePreferredWidth))
	);

	// Calculate max widths for the resizers
	const leftMaxWidth = $derived(totalAvailableWidth - middleMinWidth);
	const middleMaxWidth = $derived(totalAvailableWidth - derivedLeftWidth);
</script>

<div
	class="main-viewport"
	use:focusable={{ id: DefinedFocusable.MainViewport }}
	bind:clientWidth={containerBindWidth}
	class:middle-open={middleOpen && middle}
>
	<!-- Default layout: no swapping -->
	<div
		class="left view-wrapper"
		bind:this={leftDiv}
		style:width={derivedLeftWidth + 'rem'}
		style:min-width={leftMinWidth + 'rem'}
		use:focusable={{ id: DefinedFocusable.ViewportLeft, parentId: DefinedFocusable.MainViewport }}
	>
		<AsyncRender>
			<div class="left-content">
				{@render left()}
			</div>
			<Resizer
				viewport={leftDiv}
				direction="right"
				minWidth={leftMinWidth}
				maxWidth={leftMaxWidth}
				imitateBorder
				borderRadius={!middleOpen ? 'ml' : 'none'}
				persistId="viewport-${name}-left"
				onWidth={(width) => (leftPreferredWidth = width)}
			/>
		</AsyncRender>
	</div>

	{#if middleOpen && middle}
		<div
			class="middle view-wrapper"
			bind:this={middleDiv}
			style:width={derivedMiddleWidth + 'rem'}
			style:min-width={middleMinWidth + 'rem'}
			use:focusable={{
				id: DefinedFocusable.ViewportMiddle,
				parentId: DefinedFocusable.MainViewport
			}}
		>
			<AsyncRender>
				<div class="middle-content dotted-pattern">
					{@render middle()}
				</div>
				<Resizer
					viewport={middleDiv}
					direction="right"
					minWidth={middleMinWidth}
					maxWidth={middleMaxWidth}
					borderRadius="ml"
					persistId="viewport-${name}-middle"
					defaultValue={pxToRem(middleWidth?.default, zoom)}
					onWidth={(width) => (middlePreferredWidth = width)}
				/>
			</AsyncRender>
		</div>
	{/if}

	<div
		class="right view-wrapper dotted-pattern"
		bind:this={rightDiv}
		style:min-width={flexibleMinWidth + 'rem'}
		use:focusable={{
			id: DefinedFocusable.ViewportRight,
			parentId: DefinedFocusable.MainViewport
		}}
	>
		<AsyncRender>
			{@render right()}
		</AsyncRender>
	</div>

	{#if drawerRight}
		<div
			class="drawer-right"
			bind:this={drawerRightDiv}
			style:width={rightDrawerPreferredWidth + 'rem'}
			use:focusable={{
				id: DefinedFocusable.ViewportDrawerRight,
				parentId: DefinedFocusable.MainViewport
			}}
		>
			<AsyncRender>
				<Resizer
					viewport={drawerRightDiv}
					direction="left"
					minWidth={24}
					borderRadius="ml"
					persistId="viewport-${name}-middle"
					onWidth={(width) => (rightDrawerPreferredWidth = width)}
				/>
				{@render drawerRight()}
			</AsyncRender>
		</div>
	{/if}
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
	}

	.view-wrapper {
		display: flex;
		flex-direction: column;
		height: 100%;
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

	.left-content {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.middle {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
	}

	.middle-content {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		border-left-color: transparent;
	}

	.middle-open .left-content {
		border-radius: var(--radius-ml) 0 0 var(--radius-ml);
		border-right-color: transparent;
	}

	.right {
		position: relative;
		flex-grow: 1;
		flex-shrink: 1;
		flex-direction: column;
		height: 100%;
		margin-left: 8px;
		overflow-x: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.drawer-right {
		position: relative;
		flex-direction: column;
		height: 100%;
		margin-left: 8px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
