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

<div class="flex max-w-full">
	<Tray bind:branches />
	<Board {branches} on:finalize={onFinalize} />
</div>
