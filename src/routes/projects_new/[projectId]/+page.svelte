<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';

	export let data: PageData;
	let branches = data.branchData;
</script>

<div class="flex h-full">
	<Tray bind:columns={branches} />
	<Board bind:branches />
</div>
