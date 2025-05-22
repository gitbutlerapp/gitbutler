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
	import { Focusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
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

	let leftPreferredWidth = $derived(
		persisted(pxToRem(leftWidth.default), `$main_view_left_${name}`)
	);
	let rightPreferredWidth = $derived(
		persisted(pxToRem(rightWidth.default), `$main_view_right_${name}`)
	);

	let leftDiv = $state<HTMLDivElement>();
	let middleDiv = $state<HTMLDivElement>();
	let rightDiv = $state<HTMLDivElement>();

	const leftMinWidth = $derived(pxToRem(leftWidth.min));
	const rightMinWidth = $derived(pxToRem(rightWidth.min));

	// These need to stay in px since they are bound to elements.
	let containerBindWidth = $state<number>(1000); // TODO: What initial value should we give this?
	let middleClientWidth = $state<number>(540);
	let leftBindWidth = $state<number>(300);
	let rightBindWidth = $state<number>(300);

	// Total width we cannot go below.
	const padding = $derived(containerBindWidth - window.innerWidth);
	const containerMinWidth = $derived(800 - padding);

	const middleMinWidth = $derived(pxToRem(containerMinWidth) - leftMinWidth - rightMinWidth);

	// Left side max width depends on teh size of the right side.
	const leftMaxWidth = $derived(
		pxToRem(containerBindWidth) - middleMinWidth - pxToRem(rightBindWidth) - 2
	);
	// Right side has priority over the left, so its max size is the theoretical max.
	const rightMaxWidth = $derived(pxToRem(containerBindWidth) - middleMinWidth - leftMinWidth - 2);

	const derivedLeftWidth = $derived(
		Math.min(leftMaxWidth, Math.max(leftMinWidth, $leftPreferredWidth))
	);
	const derivedRightWidth = $derived(
		Math.min(rightMaxWidth, Math.max(rightMinWidth, $rightPreferredWidth))
	);
</script>

<div
	class="main-viewport"
	use:focusable={{ id: Focusable.Workspace }}
	bind:clientWidth={containerBindWidth}
>
	<div
		class="left"
		bind:this={leftDiv}
		bind:clientWidth={leftBindWidth}
		style:width={derivedLeftWidth + 'rem'}
		style:min-width={leftMinWidth + 'rem'}
		use:focusable={{ id: Focusable.WorkspaceLeft, parentId: Focusable.Workspace }}
	>
		{@render left()}
		<Resizer
			viewport={leftDiv}
			direction="right"
			minWidth={leftMinWidth}
			maxWidth={leftMaxWidth}
			borderRadius="ml"
			onWidth={(value) => leftPreferredWidth.set(value)}
		/>
	</div>
	<div
		class="middle"
		bind:this={middleDiv}
		bind:clientWidth={middleClientWidth}
		style:min-width={middleMinWidth + 'rem'}
		use:focusable={{ id: Focusable.WorkspaceMiddle, parentId: Focusable.Workspace }}
	>
		{@render middle()}
	</div>
	<div
		class="right"
		bind:this={rightDiv}
		bind:clientWidth={rightBindWidth}
		style:width={derivedRightWidth + 'rem'}
		style:min-width={rightMinWidth + 'rem'}
		use:focusable={{ id: Focusable.WorkspaceRight, parentId: Focusable.Workspace }}
	>
		{@render right()}
		<Resizer
			viewport={rightDiv}
			direction="left"
			minWidth={rightMinWidth}
			maxWidth={rightMaxWidth}
			borderRadius="ml"
			onWidth={(value) => rightPreferredWidth.set(value)}
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
