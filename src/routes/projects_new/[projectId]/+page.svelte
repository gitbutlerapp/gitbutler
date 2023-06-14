<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';

	export let data: PageData;
	let branches = data.branchData;
</script>

<div class="flex h-full w-full ">
	<div class="fixed inset-y-0 z-20 flex w-64 flex-col pt-11">
		<Tray bind:columns={branches} />
	</div>
	<div class="h-full w-full bg-red-500 pl-64">
		<Board bind:branches />
	</div>
</div>
