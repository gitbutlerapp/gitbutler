<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import { getVirtualBranches } from './vbranches';
	import { getTarget } from './targetData';
	import { Value } from 'svelte-loadable-store';
	import { Button } from '$lib/components';
	import { error } from '$lib/toasts';

	export let data: PageData;
	let { projectId, project, remoteBranches } = data;
	const targetOperations = getTarget(projectId);
	$: target =
		!$targetOperations.isLoading && !Value.isError($targetOperations.value)
			? $targetOperations.value
			: undefined;
	const virtualBranches = getVirtualBranches(projectId);
	$: branches =
		!$virtualBranches.isLoading && !Value.isError($virtualBranches.value)
			? $virtualBranches.value
			: [];
	let targetChoice: string | undefined;

	function onSetTargetClick() {
		if (!targetChoice) {
			return;
		}
		virtualBranches
			.setTarget(targetChoice)
			.then((t) => (target = t))
			.catch((e) => {
				console.log('failed to set branch', e);
				error('Failed to set target branch');
			});
	}
</script>

{#if target}
	<div class="flex w-full max-w-full">
		<Tray {branches} {projectId} {target} {virtualBranches} {targetOperations} />
		<Board {branches} {projectId} projectPath={project.path} {virtualBranches} />
	</div>
{:else}
	<div class="m-auto flex max-w-xs flex-col gap-y-4">
		<h1 class="text-lg font-bold">Set your target</h1>
		<p class="text-light-700 dark:text-dark-100">
			You need to set your target before you can start working on your project.
		</p>
		<!-- select menu of remoteBranches -->
		{#if remoteBranches.length === 0}
			<p class="text-gray-500">You don't have any remote branches.</p>
		{:else}
			<select bind:value={targetChoice}>
				{#each remoteBranches as branch}
					{#if branch == 'origin/master' || branch == 'origin/main'}
						<option value={branch} selected>{branch}</option>
					{:else}
						<option value={branch}>{branch}</option>
					{/if}
				{/each}
			</select>
			<div>
				<Button color="purple" height="small" on:click={onSetTargetClick}>Set Target</Button>
			</div>
		{/if}
	</div>
{/if}
