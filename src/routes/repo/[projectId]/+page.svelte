<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import { getVirtualBranches } from './vbranches';
	import { Value } from 'svelte-loadable-store';

	export let data: PageData;
	let { projectId, target, remoteBranches, remoteBranchesData } = data;
	const virtualBranches = getVirtualBranches(projectId);
	$: branches =
		!$virtualBranches.isLoading && !Value.isError($virtualBranches.value)
			? $virtualBranches.value
			: [];
	let targetChoice = 'origin/master'; // prob should check if it exists
</script>

{#if target}
	<div class="flex w-full max-w-full">
		<Tray {branches} {projectId} {target} remoteBranches={remoteBranchesData} {virtualBranches} />
		<Board {branches} {projectId} {virtualBranches} />
	</div>
{:else}
	<div class="m-auto flex flex-col space-y-2">
		<h1 class="text-2xl font-bold">Set your target</h1>
		<p class="text-gray-500">
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
			<button class="btn btn-primary" on:click={() => virtualBranches.setTarget(targetChoice)}
				>Set Target</button
			>
		{/if}
	</div>
{/if}
