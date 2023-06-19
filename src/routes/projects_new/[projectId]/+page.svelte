<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import type { Branch } from './types';

	export let data: PageData;
	let branches = data.branchData;

	function handleNewBranch(e: CustomEvent<Branch[]>) {
		branches.push(...e.detail);
		branches = branches;
	}
</script>

<div class="flex w-full max-w-full">
	<Tray bind:branches />
	<Board {branches} on:newBranch={handleNewBranch} />
</div>
