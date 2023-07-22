<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import { Button } from '$lib/components';
	import { BranchController } from '$lib/vbranches';
	import { Loaded } from 'svelte-loadable-store';
	import { getContext, setContext } from 'svelte';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import BottomPanel from './BottomPanel.svelte';

	export let data: PageData;
	let {
		projectId,
		vbranchStore,
		remoteBranchStore,
		targetBranchStore,
		remoteBranchNames,
		project
	} = data;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	const branchController = new BranchController(
		projectId,
		vbranchStore,
		remoteBranchStore,
		targetBranchStore
	);
	setContext(BRANCH_CONTROLLER_KEY, branchController);

	$: remoteBranches = Loaded.isValue($remoteBranchStore) ? $remoteBranchStore.value : [];
	$: target = Loaded.isValue($targetBranchStore) ? $targetBranchStore.value : undefined;
	$: branches =
		!$vbranchStore.isLoading && !Loaded.isError($vbranchStore) ? $vbranchStore.value : [];
	let targetChoice: string | undefined;

	function onSetTargetClick() {
		if (!targetChoice) {
			return;
		}
		branchController.setTarget(targetChoice);
	}
</script>

{#if target}
	<div class="flex w-full max-w-full" role="group" on:dragover|preventDefault>
		<Tray {branches} {remoteBranches} />
		<div
			class="z-50 -ml-[0.250rem] w-[0.250rem] shrink-0 cursor-col-resize hover:bg-orange-200 dark:bg-dark-1000 dark:hover:bg-orange-700"
			draggable="true"
			role="separator"
			on:drag={(e) => {
				userSettings.update((s) => ({
					...s,
					trayWidth: e.clientX
				}));
			}}
		/>
		<div class="flex w-full flex-col overflow-x-hidden">
			<Board {branches} {projectId} projectPath={$project?.path} {target} cloudEnabled={$project?.api?.sync || false} />
			<BottomPanel {target} />
		</div>
	</div>
{:else}
	<div class="m-auto flex max-w-xs flex-col gap-y-4">
		<h1 class="text-lg font-bold">Set your base branch</h1>
		<p class="text-light-700 dark:text-dark-100">
			You need to set your base branch before you can start working on your project.
		</p>
		<!-- select menu of remoteBranches -->
		{#if remoteBranchNames.length === 0}
			<p class="text-gray-500">You don't have any remote branches.</p>
		{:else}
			<select bind:value={targetChoice}>
				{#each remoteBranchNames as branch}
					{@const strippedBranch = branch.substring(13)}
					{#if strippedBranch == 'origin/master' || strippedBranch == 'origin/main'}
						<option value={strippedBranch} selected>{strippedBranch}</option>
					{:else}
						<option value={strippedBranch}>{strippedBranch}</option>
					{/if}
				{/each}
			</select>
			<div>
				<Button color="purple" height="small" on:click={onSetTargetClick}>Set base branch</Button>
			</div>
		{/if}
	</div>
{/if}
