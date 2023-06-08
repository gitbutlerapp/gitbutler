<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';

	export let data: PageData;
	let columnsData = data.columnsData;

	$: console.log(columnsData);
</script>

<div class="flex h-full gap-x-4 p-4">
	<Tray bind:columns={columnsData} />
	<Board bind:columns={columnsData} />
</div>
