import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';
import type { BranchLane } from './board';
import { subSeconds, subMinutes, subHours } from 'date-fns';

export const load: PageLoad = wrapLoadWithSentry(async () => {
	const columnsData: BranchLane[] = [
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
							name: 'foo-hunk-1',
							modified: subMinutes(new Date(), 5)
						},
						{
							id: 'h2',
							name: 'foo-hunk-2',
							modified: subSeconds(new Date(), 15)
						}
					]
				},
				{
					id: 'f2',
					path: 'src/bar.py',
					hunks: [
						{
							id: 'h3',
							name: 'bar-hunk-1',
							modified: subHours(new Date(), 2)
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
							name: 'foo-hunk-3',
							modified: subMinutes(new Date(), 32)
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
							name: 'bar-hunk-2',
							modified: subHours(new Date(), 1)
						}
					]
				}
			]
		}
	].map((column) => ({
		...column,
		files: column.files.map((file) => ({
			...file,
			hunks: file.hunks.sort((a, b) => b.modified.getTime() - a.modified.getTime())
		}))
	}));

	return {
		columnsData: columnsData
	};
});
