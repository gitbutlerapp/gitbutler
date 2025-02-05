<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import SelectionView from '$components/SelectionView.svelte';
	import StackDetails from '$components/StackDetails.svelte';
	import StackTabs from '$components/StackTabs.svelte';
	import WorktreeChanges from '$components/WorktreeChanges.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { onMount, setContext } from 'svelte';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const settingsService = getContext(SettingsService);
	const worktreeService = getContext(WorktreeService);
	const settingsStore = settingsService.appSettings;

	const { data }: { data: PageData } = $props();

	const projectId = $derived(data.projectId);
	const stackId = $derived(page.params.stackId);

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const idSelection = new IdSelection(worktreeService);
	setContext(IdSelection, idSelection);

	const trayWidthKey = $derived('defaulTrayWidth_ ' + projectId);
	const trayWidth = $derived(persisted<number>(240, trayWidthKey));

	const previewingKey = $derived('previewing_' + projectId);
	const previewing = $derived(persisted<boolean>(false, previewingKey));

	let resizeViewport = $state<HTMLElement>();

	const handleKeyDown = createKeybind({
		p: () => ($previewing = true)
	});
	const handleKeyUp = createKeybind({
		p: () => ($previewing = false)
	});
	function handleBlur() {
		$previewing = false;
	}

	/** Offset width for tabs component. */
	let width: number | undefined = $state();
	/** Content area on the right for stack details. */
	let right = $state<HTMLDivElement>();
	/** Width of content area on the right. */
	let rightWidth: number | undefined = $state();
	/** True if content area should be rounded. */
	let rounded = $state(false);

	$effect(() => {
		rounded = rightWidth !== width;
	});

	onMount(() => {
		const observer = new ResizeObserver(() => (rightWidth = right?.offsetWidth));
		observer.observe(right!);
		return () => {
			observer.disconnect();
		};
	});
</script>

<svelte:window onkeydown={handleKeyDown} onkeyup={handleKeyUp} onblur={handleBlur} />

<div class="workspace">
	<div class="left" bind:this={resizeViewport} style:width={$trayWidth + 'rem'}>
		<Resizer
			viewport={resizeViewport}
			direction="right"
			minWidth={36}
			onWidth={(value) => {
				$trayWidth = value / (16 * $userSettings.zoom);
			}}
		/>
		<WorktreeChanges {projectId} />
	</div>
	<div class="right" bind:this={right}>
		<StackTabs {projectId} selectedId={stackId} previewing={$previewing} bind:width />
		<div class="contents" class:rounded>
			{#if stackId}
				<StackDetails {projectId} {stackId} />
			{:else}
				<SelectionView {projectId} />
			{/if}
		</div>
	</div>
</div>

<style>
	.workspace {
		display: flex;
		flex: 1;
		align-items: stretch;
		height: 100%;
		gap: 14px;
		width: 100%;
		position: relative;
	}

	.left {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: flex-start;
		width: 290px;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		position: relative;
	}

	.right {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}

	.right .contents {
		border: 1px solid var(--clr-border-2);
		flex: 1;
		border-radius: 0 0 var(--radius-ml);
		overflow-y: none;

		&.rounded {
			border-radius: 0 var(--radius-ml) var(--radius-ml);
		}
	}
</style>
