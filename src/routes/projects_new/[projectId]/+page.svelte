<script lang="ts">
	import Board from './Board.svelte';
	import type { BranchLane } from './board';

	let columnsData: BranchLane[] = [
		{
			id: 'c1',
			name: 'implement new api',
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
			],
			uncommittedFiles: [
				{
					filePath: 'src/foo.py',
					hunks: [
						{ id: 'a', updated: new Date('2011-04-11T10:20:30Z'), description: 'add api' },
						{ id: 'b', updated: new Date('2011-04-11T10:25:30Z'), description: 'new user model' },
						{ id: 'c', updated: new Date('2011-04-11T10:27:30Z'), description: 'refactor' }
					]
				},
				{
					filePath: 'src/bar.py',
					hunks: [
						{ id: 'd', updated: new Date('2011-04-11T10:10:30Z'), description: 'extract function' },
						{ id: 'e', updated: new Date('2011-04-11T10:15:30Z'), description: 'things' }
					]
				},
				{
					filePath: 'src/baz.py',
					hunks: [
						{
							id: 'f',
							updated: new Date('2011-04-11T11:15:30Z'),
							description: 'database migration'
						}
					]
				}
			]
		},
		{
			id: 'c2',
			name: 'bug fixes',
			items: [
				{ id: 10, name: 'item50' },
				{ id: 11, name: 'item51' }
			],
			uncommittedFiles: [
				{
					filePath: 'src/foo.py',
					hunks: [
						{
							id: 'g',
							updated: new Date('2011-04-11T10:20:30Z'),
							description: 'correct broken things'
						}
					]
				}
			]
		},
		{
			id: 'c3',
			name: 'for later',
			items: [{ id: 13, name: 'item52' }],
			uncommittedFiles: [
				{
					filePath: '.env',
					hunks: [
						{
							id: 'h',
							updated: new Date('2011-04-10T12:20:30Z'),
							description: 'API keys for local development'
						}
					]
				}
			]
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
