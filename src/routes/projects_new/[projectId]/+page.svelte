<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import type { Branch } from './types';
	import { invoke } from '@tauri-apps/api';

	const set_target = async (params: { projectId: string; branch: string }) =>
		invoke<object>('set_target_branch', params);

	const create_branch = async (params: { projectId: string; name: string; path: string }) =>
		invoke<object>('create_virtual_branch', params);

	export let data: PageData;
	let branches = data.branchData;
	let target = data.target;
	let remote_branches = data.remote_branches;
	let projectId = data.projectId;
	let targetChoice = 'origin/master'; // prob should check if it exists

	function setTarget() {
		target = set_target({ projectId: projectId, branch: targetChoice });
		console.log(target);
	}

	function handleNewBranch(e: CustomEvent<Branch[]>) {
		console.log(e.detail);
		let name = e.detail[0].name;
		let path = e.detail[0].files[0].path;
		create_branch({ projectId: projectId, name: name, path: path });

		branches.push(...e.detail);
		branches = branches;
	}
</script>

{#if target}
	<div class="flex w-full max-w-full">
		<Tray bind:branches />
		<Board bind:branches on:newBranch={handleNewBranch} />
	</div>
{:else}
	<div class="m-auto flex flex-col space-y-2">
		<h1 class="text-2xl font-bold">Set your target</h1>
		<p class="gb-text-3">
			You need to set your target before you can start working on your project.
		</p>
		<!-- select menu of remote_branches -->
		{#if remote_branches.length === 0}
			<p class="gb-text-3">You don't have any remote branches.</p>
		{:else}
			<select bind:value={targetChoice}>
				{#each remote_branches as branch}
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
