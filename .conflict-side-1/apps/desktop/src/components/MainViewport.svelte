<!--
@component
A three way split view that manages resizing of the panels.

The left panel is set in rem units, the left-sideview has fixed width constraints,
and the mainSection panel grows as the window resizes. If the window shrinks to where
it is smaller than the sum of the preferred widths, then the derived widths adjust
down, with the left hand side shrinking before the left-sideview panel.

Persisted widths are only stored when resizing manually, meaning you can shrink
the window, then enlarge it and retain the original widths of the layout.

@example
```
<MainViewport
	name="workspace"
	leftWidth={{ default: 200, min: 100}}
	previewWidth={{ default: 200, min: 100}}
>
	{#snippet left()} {/snippet}
	{#snippet preview()} {/snippet}
	{#snippet mainSection()} {/snippet}
</MainViewport>
```
-->
<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import { focusable } from '$lib/focus/focusable';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import type { Snippet } from 'svelte';

	type Props = {
		testId?: string;
		name: string;
		left: Snippet;
		leftWidth: {
			default: number;
			min: number;
		};
		preview?: Snippet;
		previewWidth?: {
			default: number;
			min: number;
		};
		middle: Snippet;
		right?: Snippet;
		rightWidth: {
			default: number;
			min: number;
		};
	};

	const { testId, name, left, leftWidth, preview, previewWidth, middle, right, rightWidth }: Props =
		$props();

	const userSettings = inject(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	const uiState = inject(UI_STATE);
	const unassignedSidebaFolded = $derived(uiState.global.unassignedSidebaFolded);

	let leftPreferredWidth = $derived(pxToRem(leftWidth.default, zoom));
	let previewPreferredWidth = $derived(pxToRem(previewWidth?.default, zoom));
	let rightPreferredWidth = $derived(pxToRem(rightWidth.default, zoom));

	let leftDiv = $state<HTMLDivElement>();
	let previewDiv = $state<HTMLDivElement>();
	let rightDiv = $state<HTMLDivElement>();

	const leftMinWidth = $derived(pxToRem(leftWidth.min, zoom));
	const leftDefaultWidth = $derived(pxToRem(leftWidth.default, zoom));
	const previewMinWidth = $derived(preview ? pxToRem(previewWidth?.min, zoom) : 0);
	const rightMinWidth = $derived(pxToRem(rightWidth.min, zoom));

	// These need to stay in px since they are bound to elements.
	let containerBindWidth = $state<number>(1000); // TODO: What initial value should we give this?
	const containerBindWidthRem = $derived(pxToRem(containerBindWidth, zoom));

	// Total width we cannot go below.
	const padding = $derived(containerBindWidth - window.innerWidth);

	// While the minimum window width is 1000px we use a slightly smaller value
	// here since it happens in dev mode that the window gets smaller.
	const containerMinWidth = $derived(pxToRem(800 - padding, zoom));

	// Sum of all inner margins that cannot be used by container widths.
	const marginSum = 1;

	const middleMinWidth = $derived(
		containerMinWidth - leftMinWidth - pxToRem(previewWidth?.min, zoom) - rightMinWidth - marginSum
	);

	const leftMaxWidth = $derived(
		containerBindWidthRem - previewMinWidth - middleMinWidth - rightMinWidth - marginSum
	);

	// Calculate derived widths with proper constraints
	const finalLeftWidth = $derived(
		Math.min(
			containerBindWidthRem - previewMinWidth - middleMinWidth - rightMinWidth - marginSum,
			Math.max(leftMinWidth, leftPreferredWidth)
		)
	);

	const previewMaxWidth = $derived(
		containerBindWidthRem - finalLeftWidth - middleMinWidth - rightMinWidth - marginSum
	);

	const remainingForPreview = $derived(
		containerBindWidthRem - finalLeftWidth - middleMinWidth - rightMinWidth
	);
	const finalPreviewWidth = $derived(
		preview ? Math.min(remainingForPreview, Math.max(previewMinWidth, previewPreferredWidth)) : 0
	);

	const remainingForRight = $derived(
		containerBindWidthRem - finalLeftWidth - finalPreviewWidth - middleMinWidth - marginSum
	);
	const finalRightWidth = $derived(
		Math.min(remainingForRight, Math.max(rightMinWidth, rightPreferredWidth))
	);

	const rightMaxWidth = $derived(
		containerBindWidthRem - finalLeftWidth - finalPreviewWidth - middleMinWidth - 1
	);
</script>

<div
	class="main-viewport"
	use:focusable
	bind:clientWidth={containerBindWidth}
	data-testid={testId}
	class:left-sideview-open={!!preview}
>
	{#if !unassignedSidebaFolded.current}
		<div
			class="left-section view-wrapper"
			bind:this={leftDiv}
			style:width={finalLeftWidth + 'rem'}
			style:min-width={leftMinWidth + 'rem'}
			use:focusable={{ list: true }}
		>
			<div class="left-section__content">
				{@render left()}
			</div>
			<Resizer
				viewport={leftDiv}
				direction="right"
				minWidth={leftMinWidth}
				defaultValue={leftDefaultWidth}
				maxWidth={leftMaxWidth}
				persistId="viewport-${name}-left-section"
				onWidth={(width) => {
					leftPreferredWidth = width;
				}}
			/>
		</div>

		{#if preview}
			<div
				class="left-sideview view-wrapper"
				bind:this={previewDiv}
				style:width={finalPreviewWidth + 'rem'}
				style:min-width={previewMinWidth + 'rem'}
				use:focusable
			>
				<div class="left-sideview-content dotted-pattern">
					{@render preview()}
				</div>
				<Resizer
					viewport={previewDiv}
					direction="right"
					minWidth={previewMinWidth}
					maxWidth={previewMaxWidth}
					persistId="viewport-${name}-left-sideview"
					defaultValue={pxToRem(previewWidth?.default, zoom)}
					onWidth={(width) => {
						previewPreferredWidth = width;
					}}
				/>
			</div>
		{/if}
	{:else}
		<div class="left-section__folded">
			{@render left()}
		</div>
	{/if}

	<div
		class="main-section view-wrapper dotted-pattern"
		style:min-width={middleMinWidth + 'rem'}
		style:margin-right={right ? '0' : ''}
	>
		{@render middle()}
	</div>

	{#if right}
		<div
			class="right-sideview"
			bind:this={rightDiv}
			style:width={finalRightWidth + 'rem'}
			use:focusable
		>
			<Resizer
				viewport={rightDiv}
				direction="left"
				minWidth={rightMinWidth}
				defaultValue={pxToRem(rightWidth.default, zoom)}
				maxWidth={rightMaxWidth}
				persistId="viewport-${name}-right-sideview"
				onWidth={(width) => {
					rightPreferredWidth = width;
				}}
			/>
			<div class="right-sideview-content">
				{@render right()}
			</div>
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
