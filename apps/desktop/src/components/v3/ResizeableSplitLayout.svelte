<!--
  @component

  A split layout reusable component that gives you the ability to
  resize a side thing, while not having to re-declare resizing
  functionality.

  You must pass a template for the main section, and optionally a left
  fragment, a right fragment, but not both.
-->
<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import type { Snippet } from 'svelte';

	type Props = {
		main: Snippet;
		left?: Snippet;
		right?: Snippet;
		projectId: string;
	};

	const { main, left, right, projectId }: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const resizerId = `sideResizer_${projectId}`;
	const persistedWidth = persisted(20, resizerId);

	let leftViewport: HTMLDivElement | undefined = $state();
	let rightViewport: HTMLDivElement | undefined = $state();
</script>

<div class="common-editor-layout">
	{#if left}
		<div class="left" bind:this={leftViewport} style:width={$persistedWidth + 'rem'}>
			<Resizer
				viewport={leftViewport}
				direction="right"
				minWidth={240}
				onWidth={(value) => {
					$persistedWidth = value / (16 * $userSettings.zoom);
				}}
			/>
			{@render left()}
		</div>
	{/if}
	<div class="main">
		{@render main()}
	</div>
	{#if right}
		<div class="right" bind:this={rightViewport} style:width={$persistedWidth + 'rem'}>
			<Resizer
				viewport={rightViewport}
				direction="left"
				minWidth={240}
				onWidth={(value) => {
					$persistedWidth = value / (16 * $userSettings.zoom);
				}}
			/>
			{@render right()}
		</div>
	{/if}
</div>

<style>
	.common-editor-layout {
		display: flex;
		flex-grow: 1;
		max-width: 100%;
		height: 100%;
	}
	.main {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		height: 100%;
	}
	.left,
	.right {
		position: relative;
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
		padding: 14px;
		flex-shrink: 0;
	}
	.left {
		border-right: 1px solid var(--clr-border-2);
	}
	.right {
		border-left: 1px solid var(--clr-border-2);
	}
</style>
