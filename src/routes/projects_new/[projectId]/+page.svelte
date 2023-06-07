<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { BranchLane } from './board';

	let columnsData: BranchLane[] = [
		{
			id: 'c1',
			name: 'feature-1',
			active: true,
			files: [
				{
					id: 'f1',
					path: 'src/foo.py',
					hunks: [
						{
							id: 'h1',
							name: 'foo-hunk-1'
						},
						{
							id: 'h2',
							name: 'foo-hunk-2'
						}
					]
				},
				{
					id: 'f2',
					path: 'src/bar.py',
					hunks: [
						{
							id: 'h3',
							name: 'bar-hunk-1'
						}
					]
				}
			]
		},
		{
			id: 'c2',
			name: 'bugfix',
			active: true,
			files: [
				{
					id: 'f3',
					path: 'src/foo.py',
					hunks: [
						{
							id: 'h4',
							name: 'foo-hunk-3'
						}
					]
				}
			]
		},
		{
			id: 'c3',
			name: 'stashed-things',
			active: false,
			files: [
				{
					id: 'f4',
					path: 'src/bar.py',
					hunks: [
						{
							id: 'h5',
							name: 'bar-hunk-2'
						}
					]
				}
			]
		}
	];
	$: console.log(columnsData);
</script>

<div class="flex h-full gap-x-4 p-4">
	<Tray bind:columns={columnsData} />
	<Board bind:columns={columnsData} />
</div>
