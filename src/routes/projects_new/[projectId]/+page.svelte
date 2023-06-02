<script lang="ts">
	import Board from './Board.svelte';
	import type { BranchLane } from './board';

	let columnsData: BranchLane[] = [
		{
			id: 'c1',
			name: 'TODO',
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
			items: [
				{ id: 10, name: 'item50' },
				{ id: 11, name: 'item51' }
			]
		},
		{
			id: 'c3',
			name: 'DONE',
			items: [{ id: 13, name: 'item52' }]
		}
	];
	function handleBoardUpdated(newColumnsData: BranchLane[]) {
		// TODO: API calls
		columnsData = newColumnsData;
	}
</script>

<Board columns={columnsData} onFinalUpdate={handleBoardUpdated} />

<style>
	:global(*) {
		box-sizing: border-box;
		margin: 0;
	}
</style>
