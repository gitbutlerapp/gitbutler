<script lang="ts">
	import SelectionView from './SelectionView.svelte';
	import Resizer from '$components/Resizer.svelte';
	import StackTabs from '$components/v3/StackTabs.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { onMount, type Snippet } from 'svelte';
	import { page } from '$app/state';

	interface Props {
		projectId: string;
		stackId?: string;
		branchName?: string;
		children: Snippet;
	}

	const { stackId, projectId, branchName, children }: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	const trayWidthKey = $derived('defaulTrayWidth_ ' + projectId);
	const trayWidth = $derived(persisted<number>(240, trayWidthKey));

	const previewingKey = $derived('previewing_' + projectId);
	const previewing = $derived(persisted<boolean>(false, previewingKey));

	let resizeViewport = $state<HTMLElement>();

	/** Offset width for tabs component. */
	let width = $state<number>();
	/** Content area on the right for stack details. */
	let rightEl = $state<HTMLDivElement>();
	/** Width of content area on the right. */
	let rightWidth = $state<number>();
	/** True if content area should be rounded. */
	const rounded = $derived(rightWidth !== width);

	const [selection] = inject(IdSelection);
	const selectedIds = $derived(selection.values());
	const previewMode = $derived(page.url.searchParams.get('preview') && selectedIds.length > 0);

	onMount(() => {
		const observer = new ResizeObserver(() => (rightWidth = rightEl?.offsetWidth));
		observer.observe(rightEl!);
		return () => {
			observer.disconnect();
		};
	});
</script>

<div class="stack-view">
	<div class="left" bind:this={resizeViewport} style:width={$trayWidth + 'rem'}>
		<WorktreeChanges {projectId} {stackId} {branchName} />
		<Resizer
			viewport={resizeViewport}
			direction="right"
			minWidth={240}
			onWidth={(value) => {
				$trayWidth = value / (16 * $userSettings.zoom);
			}}
		/>
	</div>
	<div class="right" bind:this={rightEl}>
		<StackTabs {projectId} selectedId={stackId} previewing={$previewing} bind:width />
		<div class="contents" class:rounded>
			{#if previewMode}
				<SelectionView {projectId} />
			{:else}
				{@render children()}
			{/if}
		</div>
	</div>
</div>

<style>
	.stack-view {
		display: flex;
		flex: 1;
		gap: 14px;
		align-items: stretch;
		height: 100%;
		width: 100%;
		position: relative;
	}

	.left {
		height: 100%;
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		position: relative;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		/* Resizer looks better with hidden overflow. */
		overflow: hidden;
	}

	.right {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		min-width: 10rem;
	}

	.right .contents {
		display: flex;
		border: 1px solid var(--clr-border-2);
		flex: 1;
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		overflow: hidden;

		&.rounded {
			border-radius: 0 var(--radius-ml) var(--radius-ml) var(--radius-ml);
		}
	}
</style>
