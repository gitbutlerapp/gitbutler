<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		title?: string;
		stackId: string;
		header?: Snippet;
		extraActions?: Snippet;
		children: Snippet;
	};

	const { header, title, extraActions, children, projectId, stackId }: Props = $props();

	const [uiState] = inject(UiState);

	const projectUiState = $derived(uiState.project(projectId));
	const stackUiState = $derived(uiState.stack(stackId));

	const drawerIsFullScreen = $derived(projectUiState.drawerFullScreen.get());
	const heightRmResult = $derived(uiState.global.drawerHeight.get());
	const heightRm = $derived(`min(${heightRmResult.current}rem, 80%)`);
	const height = $derived(drawerIsFullScreen.current ? '100%' : heightRm);

	let drawerDiv = $state<HTMLDivElement>();

	function onToggleExpand() {
		projectUiState.drawerFullScreen.set(!drawerIsFullScreen.current);
	}

	export function onClose() {
		projectUiState.drawerPage.set(undefined);
		stackUiState.selection.set(undefined);
	}
</script>

<div class="drawer" bind:this={drawerDiv} style:height>
	<div class="drawer-header">
		<div class="drawer-header__main">
			{#if title}
				<h3 class="text-15 text-bold">
					{title}
				</h3>
			{/if}
			{#if header}
				{@render header()}
			{/if}
		</div>

		<div class="drawer-header__actions">
			{#if extraActions}
				{@render extraActions()}
			{/if}
			<Button
				kind="ghost"
				icon={drawerIsFullScreen.current ? 'chevron-down' : 'chevron-up'}
				size="tag"
				onclick={onToggleExpand}
			/>
			<Button kind="ghost" icon="cross" size="tag" onclick={onClose} />
		</div>
	</div>

	{#if children}
		{@render children()}
	{/if}

	{#if !drawerIsFullScreen.current}
		<Resizer
			direction="up"
			viewport={drawerDiv}
			minHeight={11}
			onHeight={(value) => uiState.global.drawerHeight.set(value)}
		/>
	{/if}
</div>

<style>
	.drawer {
		overflow: hidden;
		position: relative;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		flex-grow: 1;
		width: 100%;
		padding: 14px;
		box-sizing: border-box;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.drawer-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 12px;
	}

	.drawer-header__main {
		flex-grow: 1;
		display: flex;
		gap: 8px;
	}

	.drawer-header__actions {
		flex-shrink: 0;
		display: flex;
		gap: 2px;
		margin-top: -4px;
		margin-right: -4px;
	}
</style>
