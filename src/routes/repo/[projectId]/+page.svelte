<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import { Button } from '$lib/components';
	import { BranchController } from '$lib/vbranches';
	import { Loaded } from 'svelte-loadable-store';

	export let data: PageData;
	let {
		projectId,
		vbranchStore,
		remoteBranchStore,
		targetBranchStore,
		remoteBranchNames,
		project,
		userSettings
	} = data;

	const branchController = new BranchController(
		projectId,
		vbranchStore,
		remoteBranchStore,
		targetBranchStore
	);

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
	<div class="flex w-full max-w-full">
		<Tray {branches} {target} {branchController} {remoteBranches} {userSettings} />
		<Board {branches} {projectId} projectPath={project.path} {branchController} {userSettings} />
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
					{#if branch == 'origin/master' || branch == 'origin/main'}
						<option value={branch} selected>{branch}</option>
					{:else}
						<option value={branch}>{branch}</option>
					{/if}
				{/each}
			</select>
			<div>
				<Button color="purple" height="small" on:click={onSetTargetClick}>Set base branch</Button>
			</div>
		{/if}
	</div>
{/if}
