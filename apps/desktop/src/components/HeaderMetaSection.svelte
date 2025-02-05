<script lang="ts">
	import SeriesRowLabels from './SeriesLabels.svelte';
	import BranchLaneContextMenu from '$components/BranchLaneContextMenu.svelte';
	import { PatchSeries } from '$lib/branches/branch';
	import { cloudReviewFunctionality } from '$lib/config/uiFeatureFlags';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';

	interface Props {
		series: (PatchSeries | Error)[];
		onCollapseButtonClick: () => void;
		stackId?: string;
	}

	const { series, onCollapseButtonClick, stackId }: Props = $props();

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabButtonEl: HTMLButtonElement | undefined = $state();
	let isContextMenuOpen = $state(false);

	const stackPublishingService = getContext(StackPublishingService);
	const canPublish = stackPublishingService.canPublish;
	let publishing = $state<'inert' | 'loading' | 'complete'>('inert');

	async function publishStack() {
		publishing = 'loading';
		await stackPublishingService.upsertStack(stackId);
		publishing = 'complete';
	}
</script>

<div class="stack-meta">
	<div class="stack-meta-top">
		<SeriesRowLabels {series} />

		<Button
			bind:el={kebabButtonEl}
			activated={isContextMenuOpen}
			kind="ghost"
			icon="kebab"
			size="tag"
			onclick={() => {
				contextMenu?.toggle();
			}}
		/>
		<BranchLaneContextMenu
			bind:contextMenuEl={contextMenu}
			trigger={kebabButtonEl}
			onCollapse={onCollapseButtonClick}
			ontoggle={(isOpen) => (isContextMenuOpen = isOpen)}
		/>
	</div>
	{#if $cloudReviewFunctionality && $canPublish}
		<div class="stack-meta-bottom">
			<Button wide onclick={publishStack} loading={publishing === 'loading'}>Publish stack</Button>
		</div>
	{/if}
</div>

<style lang="postcss">
	.stack-meta {
		width: 100%;
		align-items: start;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-top: none;
	}

	.stack-meta-top {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px;
	}

	.stack-meta-bottom {
		width: 100%;
		display: flex;
		padding: 0 12px 12px 12px;
	}
</style>
