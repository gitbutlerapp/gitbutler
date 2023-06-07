<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { BranchLane } from './board';

	let columnsData: BranchLane[] = [
		{
			id: 'c1',
			name: 'TODO',
			active: true,
			items: [
				{ id: 1, name: 'item41' },
				{ id: 2, name: 'item42' },
				{ id: 3, name: 'item43' },
				{ id: 4, name: 'item44' },
				{ id: 5, name: 'item45' },
				{ id: 6, name: 'item46' },
				{ id: 7, name: 'item47' },
				{ id: 8, name: 'item48' },
				{ id: 9, name: 'item49' }
			]
		},
		{
			id: 'c2',
			name: 'DOING',
			active: true,
			items: [
				{ id: 10, name: 'item50' },
				{ id: 11, name: 'item51' }
			]
		},
		{
			id: 'c3',
			name: 'DONE',
			active: false,
			items: [{ id: 13, name: 'item52' }]
		}
	];
	$: console.log(columnsData);
</script>

<div class="flex h-full gap-x-4 p-4">
	<Tray bind:columns={columnsData} />
	<Board bind:columns={columnsData} />
</div>
