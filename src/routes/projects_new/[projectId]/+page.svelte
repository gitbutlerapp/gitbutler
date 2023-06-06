<script lang="ts">
	import Board from './Board.svelte';
	import type { BranchLane } from './board';

	let columnsData: BranchLane[] = [
		{
			id: 'c1',
			name: 'new user api',
			hunks: [
				{
					id: 'h1',
					filePath: 'src/foo.py',
					description: 'add new user api',
					diff: '...',
					modified: new Date('2011-04-11T10:20:30Z'),
					commitId: null
				},
				{
					id: 'h2',
					filePath: 'src/foo.py',
					description: 'refactor things',
					diff: '...',
					modified: new Date('2011-04-11T10:22:30Z'),
					commitId: null
				},
				{
					id: 'h3',
					filePath: 'src/foo.py',
					description: 'fix bug',
					diff: '...',
					modified: new Date('2011-04-11T10:29:30Z'),
					commitId: null
				},
				{
					id: 'h4',
					filePath: 'src/bar.py',
					description: 'something else',
					diff: '...',
					modified: new Date('2011-04-11T10:27:30Z'),
					commitId: null
				},
				{
					id: 'h5',
					filePath: 'src/bar.py',
					description: 'database migration',
					diff: '...',
					modified: new Date('2011-04-11T10:28:30Z'),
					commitId: null
				},
				{
					id: 'h6',
					filePath: 'src/baz.py',
					description: 'messing around',
					diff: '...',
					modified: new Date('2011-04-11T10:20:30Z'),
					commitId: null
				},
				{
					id: 'h7',
					filePath: '.env',
					description: 'development api keys',
					diff: '...',
					modified: new Date('2011-04-11T10:28:30Z'),
					commitId: null
				}
			]
		},
		{
			id: 'c2',
			name: 'fix random bug',
			hunks: []
		},
		{
			id: 'c3',
			name: 'stuff for later',
			hunks: []
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
