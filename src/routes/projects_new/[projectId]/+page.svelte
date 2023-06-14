<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import type { Branch } from './types';

	export let data: PageData;
	let branches = data.branchData;

	// We only want the tray to update on finalized changes.
	function onFinalize(e: CustomEvent<Branch[]>) {
		branches = e.detail;
	}
</script>

<div class="flex h-full w-full ">
	<div class="fixed inset-y-0 z-20 flex w-64 flex-col pt-11">
		<Tray bind:branches />
	</div>
	<div class="h-full w-full pl-64">
		<Board {branches} on:finalize={onFinalize} />
	</div>
</div>
