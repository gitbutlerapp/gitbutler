<!--
@component
A three way split view that manages resizing of the panels.

The leftSection panel is set in rem units, the left-sideview has fixed width constraints,
and the mainSection panel grows as the window resizes. If the window shrinks to where 
it is smaller than the sum of the preferred widths, then the derived widths adjust 
down, with the leftSection hand side shrinking before the left-sideview panel.

Persisted widths are only stored when resizing manually, meaning you can shrink
the window, then enlarge it and retain the original widths of the layout.

@example
```
<MainViewport
	name="workspace"
	leftSectionWidth={{ default: 200, min: 100}}
	leftSideviewWidth={{ default: 200, min: 100}}
>
	{#snippet leftSection()} {/snippet}
	{#snippet leftSideview()} {/snippet}
	{#snippet mainSection()} {/snippet}
</MainViewport>
```
-->
<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import AsyncRender from '$components/v3/AsyncRender.svelte';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { inject } from '@gitbutler/shared/context';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import type { Snippet } from 'svelte';

	type Props = {
		name: string;
		leftSection: Snippet;
		leftSideview?: Snippet;
		mainSection: Snippet;
		rightSideview?: Snippet;
		leftSectionWidth: {
			default: number;
			min: number;
		};
		leftSideviewWidth?: {
			default: number;
			min: number;
		};
	};

	const {
		name,
		leftSection,
		leftSideview,
		mainSection,
		rightSideview,
		leftSectionWidth,
		leftSideviewWidth
	}: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	const [uiState] = inject(UiState);
	const unassignedSidebaFolded = $derived(uiState.global.unassignedSidebaFolded);

	const defaultRightSideviewWidth = 480;
	let leftSectionPreferredWidth = $derived(pxToRem(leftSectionWidth.default, zoom));
	let leftSideviewPreferredWidth = $derived(pxToRem(leftSideviewWidth?.default, zoom));
	let rightSideviewPreferredWidth = $derived(pxToRem(defaultRightSideviewWidth, zoom));

	let leftSectionDiv = $state<HTMLDivElement>();
	let leftSideviewDiv = $state<HTMLDivElement>();
	let mainSectionDiv = $state<HTMLDivElement>();
	let rightSideviewDiv = $state<HTMLDivElement>();

	const leftSectionMinWidth = $derived(pxToRem(leftSectionWidth.min, zoom));
	const leftSideviewMinWidth = $derived(pxToRem(leftSideviewWidth?.min, zoom));
	const rightSideviewMinWidth = $derived(pxToRem(400, zoom));

	// These need to stay in px since they are bound to elements.
	let containerBindWidth = $state<number>(1000); // TODO: What initial value should we give this?

	// Total width we cannot go below.
	const padding = $derived(containerBindWidth - window.innerWidth);
	const containerMinWidth = $derived(804 - padding);

	// When swapped, the left-sideview becomes flexible and mainSection becomes fixed
	const rightSideviewWidth = $derived(rightSideview ? rightSideviewPreferredWidth : 0);
	const flexibleMinWidth = $derived(
		pxToRem(containerMinWidth, zoom) -
			leftSectionMinWidth -
			(leftSideviewMinWidth || 0) -
			rightSideviewWidth
	);
	const totalAvailableWidth = $derived(
		pxToRem(containerBindWidth, zoom) - flexibleMinWidth - rightSideviewWidth - 1
	); // Reserve space for flexible panel, right sideview and gaps

	// Calculate derived widths with proper constraints
	const derivedLeftSectionWidth = $derived(
		Math.min(
			totalAvailableWidth - leftSideviewMinWidth,
			Math.max(leftSectionMinWidth, leftSectionPreferredWidth)
		)
	);

	// Fixed panel width is constrained by remaining space after leftSection panel
	const remainingForFixed = $derived(totalAvailableWidth - derivedLeftSectionWidth);
	const derivedLeftSideviewWidth = $derived(
		Math.min(remainingForFixed, Math.max(leftSideviewMinWidth, leftSideviewPreferredWidth))
	);

	// Calculate max widths for the resizers
	const leftSectionMaxWidth = $derived(totalAvailableWidth - leftSideviewMinWidth);
	const leftSideviewMaxWidth = $derived(totalAvailableWidth - derivedLeftSectionWidth);
</script>

<div
	class="main-viewport"
	use:focusable={{ id: DefinedFocusable.MainViewport }}
	bind:clientWidth={containerBindWidth}
	class:left-sideview-open={!!leftSideview}
>
	{#if !unassignedSidebaFolded.current}
		<div
			class="left-section view-wrapper"
			bind:this={leftSectionDiv}
			style:width={derivedLeftSectionWidth + 'rem'}
			style:min-width={leftSectionMinWidth + 'rem'}
			use:focusable={{ id: DefinedFocusable.ViewportLeft, parentId: DefinedFocusable.MainViewport }}
		>
			<AsyncRender>
				<div class="left-section__content">
					{@render leftSection()}
				</div>
				<Resizer
					viewport={leftSectionDiv}
					direction="right"
					minWidth={leftSectionMinWidth}
					maxWidth={leftSectionMaxWidth}
					imitateBorder
					borderRadius={!leftSideview ? 'ml' : 'none'}
					persistId="viewport-${name}-left-section"
					onWidth={(width) => (leftSectionPreferredWidth = width)}
				/>
			</AsyncRender>
		</div>

		{#if leftSideview}
			<div
				class="left-sideview view-wrapper"
				bind:this={leftSideviewDiv}
				style:width={derivedLeftSideviewWidth + 'rem'}
				style:min-width={leftSideviewMinWidth + 'rem'}
				use:focusable={{
					id: DefinedFocusable.ViewportMiddle,
					parentId: DefinedFocusable.MainViewport
				}}
			>
				<AsyncRender>
					<div class="left-sideview-content dotted-pattern">
						{@render leftSideview()}
					</div>
					<Resizer
						viewport={leftSideviewDiv}
						direction="right"
						minWidth={leftSideviewMinWidth}
						maxWidth={leftSideviewMaxWidth}
						borderRadius="ml"
						persistId="viewport-${name}-left-sideview"
						defaultValue={pxToRem(leftSideviewWidth?.default, zoom)}
						onWidth={(width) => (leftSideviewPreferredWidth = width)}
					/>
				</AsyncRender>
			</div>
		{/if}
	{:else}
		<div class="left-section__folded">
			{@render leftSection()}
		</div>
	{/if}

	<div
		class="main-section view-wrapper dotted-pattern"
		bind:this={mainSectionDiv}
		style:min-width={flexibleMinWidth + 'rem'}
		style:margin-right={rightSideview ? '0' : ''}
		use:focusable={{
			id: DefinedFocusable.ViewportRight,
			parentId: DefinedFocusable.MainViewport
		}}
	>
		<AsyncRender>
			{@render mainSection()}
		</AsyncRender>
	</div>

	{#if rightSideview}
		<div
			class="right-sideview"
			bind:this={rightSideviewDiv}
			style:width={rightSideviewPreferredWidth + 'rem'}
			use:focusable={{
				id: DefinedFocusable.ViewportDrawerRight,
				parentId: DefinedFocusable.MainViewport
			}}
		>
			<AsyncRender>
				<Resizer
					viewport={rightSideviewDiv}
					direction="left"
					minWidth={rightSideviewMinWidth}
					defaultValue={pxToRem(defaultRightSideviewWidth, zoom)}
					borderRadius="ml"
					persistId="viewport-${name}-right-sideview"
					onWidth={(width) => (rightSideviewPreferredWidth = width)}
				/>
				<div class="right-sideview-content">
					{@render rightSideview()}
				</div>
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

	.left-section {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
	}

	.left-section__content {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.left-section__folded {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		width: 44px;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.left-sideview {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
	}

	.left-sideview-content {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-left-width: 0;
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		border-left-color: transparent;
	}

	.left-sideview-open .left-section__content {
		border-radius: var(--radius-ml) 0 0 var(--radius-ml);
		border-right-color: transparent;
	}

	.main-section {
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

	.right-sideview {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
		margin-left: 8px;
	}

	.right-sideview-content {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
