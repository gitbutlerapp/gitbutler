<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import type { Branch } from './types';
	import { invoke } from '@tauri-apps/api';

	export let data: PageData;
	let { projectId, target, branches, remoteBranches, remoteBranchesData } = data;
	let targetChoice = 'origin/master'; // prob should check if it exists

	async function setTarget() {
		return invoke<object>('set_target_branch', {
			projectId: projectId,
			branch: targetChoice
		});
	}

	async function createBranch(params: { projectId: string; name: string; path: string }) {
		return invoke<object>('create_virtual_branch', params);
	}

	function handleNewBranch(e: CustomEvent<Branch[]>) {
		let name = e.detail[0].name;
		let path = e.detail[0].files[0].path;
		createBranch({ projectId: projectId, name: name, path: path });
		branches.push(...e.detail);
	}
</script>

{#if target}
	<div class="flex w-full max-w-full">
		<Tray bind:branches {target} remoteBranches={remoteBranchesData} />
		<Board {projectId} bind:branches on:newBranch={handleNewBranch} />
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
			<button class="btn btn-primary" on:click={setTarget}>Set Target</button>
		{/if}
	</div>
{/if}
