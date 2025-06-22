<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		title?: string;
		header?: Snippet;
		extraActions?: Snippet;
		kebabMenu?: Snippet<[element: HTMLElement]>;
		children: Snippet;
		filesSplitView?: Snippet;
		headerNoPaddingLeft?: boolean;
		testId?: string;
		onclose?: () => void;
	};

	const {
		title,
		header,
		extraActions,
		kebabMenu,
		children,
		filesSplitView,
		headerNoPaddingLeft,
		testId,
		onclose
	}: Props = $props();

	let headerDiv = $state<HTMLDivElement>();
</script>

<div data-testid={testId} class="drawer">
	<div bind:this={headerDiv} class="drawer-header" class:no-padding-left={headerNoPaddingLeft}>
		<div class="drawer-header__title">
			{#if title}
				<h3 class="text-15 text-bold truncate">
					{title}
				</h3>
			{/if}
			{#if header}
				{@render header()}
			{/if}
		</div>

		<div class="drawer-header__actions">
			{#if extraActions}
				<div class="drawer-header__actions-group">
					{@render extraActions()}
				</div>
			{/if}
			<div class="drawer-header__actions-group">
				{#if kebabMenu}
					{@render kebabMenu(headerDiv)}
				{/if}

				{#if onclose}
					<Button kind="ghost" icon="cross" size="tag" onclick={() => onclose()} />
				{/if}
			</div>
		</div>
	</div>

	<ConfigurableScrollableContainer>
		{#if children}
			<div class="drawer__content">
				{@render children()}
			</div>

			{#if filesSplitView}
				<div class="drawer__files-split-view">
					{@render filesSplitView()}
				</div>
			{/if}
		{/if}
	</ConfigurableScrollableContainer>
</div>

<style>
	.drawer {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		max-height: calc(100% + 1px);
		overflow: hidden;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.drawer-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 42px;
		padding: 0 8px 0 14px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);

		&.no-padding-left {
			padding-left: 0;
		}
	}

	.drawer-header__title {
		display: flex;
		flex-grow: 1;
		align-items: center;
		height: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.drawer-header__actions {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 12px;
	}

	.drawer-header__actions-group {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.drawer__content {
		container-name: drawer-content;
		container-type: inline-size;
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 14px;
		user-select: text;
	}

	.drawer__files-split-view {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}
</style>
