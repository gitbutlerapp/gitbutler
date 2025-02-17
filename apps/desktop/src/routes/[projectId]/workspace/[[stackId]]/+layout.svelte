<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import StackTabs from '$components/v3/StackTabs.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { onMount, setContext, type Snippet } from 'svelte';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const settingsService = getContext(SettingsService);
	const worktreeService = getContext(WorktreeService);
	const settingsStore = settingsService.appSettings;

	const { data, children }: { data: PageData; children: Snippet } = $props();

	const projectId = $derived(data.projectId);
	const stackId = $derived(page.params.stackId);
	const stackService = getContext(StackService);
	const result = $derived(stackService.getStacks(projectId));

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});

	$effect(() => {
		if (!stackId && result.current.data?.[0]) {
			const firstStackId = result.current.data?.[0]?.id;
			goto(`/${data.projectId}/workspace/${firstStackId}`);
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
	let rightEl = $state<HTMLDivElement>();
	/** Width of content area on the right. */
	let rightWidth: number | undefined = $state();
	/** True if content area should be rounded. */
	let rounded = $state(false);

	$effect(() => {
		rounded = rightWidth !== width;
	});

	onMount(() => {
		const observer = new ResizeObserver(() => (rightWidth = rightEl?.offsetWidth));
		observer.observe(rightEl!);
		return () => {
			observer.disconnect();
		};
	});
</script>

<svelte:window onkeydown={handleKeyDown} onkeyup={handleKeyUp} onblur={handleBlur} />

<div class="workspace">
	<div class="left">
		<div class="resizable-area" bind:this={resizeViewport} style:width={$trayWidth + 'rem'}>
			<WorktreeChanges {projectId} {stackId} />
		</div>
		<Resizer
			viewport={resizeViewport}
			direction="right"
			minWidth={36}
			onWidth={(value) => {
				$trayWidth = value / (16 * $userSettings.zoom);
			}}
		/>
	</div>
	<div class="right" bind:this={rightEl}>
		<StackTabs {projectId} selectedId={stackId} previewing={$previewing} bind:width />
		<div class="contents" class:rounded>
			{@render children()}
		</div>
	</div>
</div>

<style>
	.workspace {
		display: flex;
		flex: 1;
		align-items: stretch;
		height: 100%;
		width: 100%;
		position: relative;
	}

	.left {
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		overflow: hidden;
		position: relative;
		padding-right: 8px;
	}

	.resizable-area {
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		height: 100%;
	}

	.right {
		display: flex;
		flex: 1;
		margin-left: 6px;
		flex-direction: column;
		overflow: hidden;
	}

	.right .contents {
		border: 1px solid var(--clr-border-2);
		flex: 1;
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		overflow: hidden;

		&.rounded {
			border-radius: 0 var(--radius-ml) var(--radius-ml) var(--radius-ml);
		}
	}
</style>
