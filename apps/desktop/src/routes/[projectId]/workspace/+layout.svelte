<script lang="ts">
	import WorkspaceView from '$components/v3/WorkspaceView.svelte';
	import StackTabs from '$components/v3/stackTabs/StackTabs.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { remToPx } from '@gitbutler/ui/utils/remToPx';
	import type { PageData } from './$types';
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const stackService = getContext(StackService);
	const settingsService = getContext(SettingsService);
	const modeService = getContext(ModeService);
	const settingsStore = settingsService.appSettings;
	const mode = modeService.mode;

	const { data, children }: { data: PageData; children: Snippet } = $props();

	const projectId = $derived(page.params.projectId);
	const stackId = $derived(page.params.stackId);

	const stacks = $derived(projectId ? stackService.stacks(projectId) : undefined);

	/** Offset width for tabs component. */
	let tabsWidth = $state<number>();

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});

	$effect(() => {
		// If the data is loading, do nothing
		if (!stacks?.current.data) return;
		const stackFoundWithCurrentPageId = stacks?.current.data?.some((stack) => stack.id === stackId);
		// If we are on a valid stack, do nothing
		if (stackFoundWithCurrentPageId) return;

		if (stacks.current.data.length === 0) {
			goto(`/${data.projectId}/workspace`);
		} else {
			goto(`/${data.projectId}/workspace/${stacks.current.data[0]!.id}`);
		}
	});

	function gotoEdit() {
		goto(`/${projectId}/edit`);
	}

	$effect(() => {
		if ($mode?.type === 'Edit') {
			// That was causing an incorrect linting error when project.id was accessed inside the reactive block
			gotoEdit();
		}
	});
</script>

{#if projectId}
	<WorkspaceView {projectId} {stackId}>
		{#snippet right({ viewportWidth })}
			<StackTabs {projectId} selectedId={stackId} bind:width={tabsWidth} />
			{#if (stacks?.current.data?.length ?? 1) > 0}
				<div
					class="contents"
					class:rounded={tabsWidth! <= (remToPx(viewportWidth - 0.5) as number)}
				>
					{@render children()}
				</div>
			{:else}
				<div
					class="no-stacks"
					class:rounded={tabsWidth! <= (remToPx(viewportWidth - 0.5) as number)}
				>
					{@html noBranchesSvg}
					<div class="no-stacks-text">
						<p class="text-15 text-bold no-stacks-title">You have no branches</p>
						<p class="text-13 no-stacks-caption">
							Create a new branch for a feature, fix, or idea!
						</p>
					</div>
				</div>
			{/if}
		{/snippet}
	</WorkspaceView>
{/if}

<style>
	.contents {
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;

		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		border: 1px solid var(--clr-border-2);

		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
	}

	.no-stacks {
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;

		align-items: center;
		justify-content: center;

		gap: 20px;

		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		border: 1px solid var(--clr-border-2);
	}

	.no-stacks-text {
		display: flex;
		flex-direction: column;
		gap: 6px;

		align-items: center;
		text-align: center;

		width: 195px;
	}

	.no-stacks-title {
		color: var(--clr-scale-ntrl-40);
	}

	.no-stacks-caption {
		color: var(--clr-text-2);
	}

	.rounded {
		border-radius: 0 var(--radius-ml) var(--radius-ml) var(--radius-ml);
	}
</style>
