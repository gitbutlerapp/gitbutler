import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';
import type { Branch } from './types';
import { subSeconds, subMinutes, subHours } from 'date-fns';

export const load: PageLoad = async () => {
	const branches: Branch[] = [
		{
			id: 'c1',
			name: 'feature-1',
			active: true,
			kind: 'lane',
			files: [
				{
					id: 'f1',
					path: 'src/foo.py',
					kind: 'file',
					hunks: [
						{
							id: 'h1',
							name: 'foo-hunk-1',
							kind: 'hunk',
							modified: subMinutes(new Date(), 5),
							filePath: 'src/foo.py'
						},
						{
							id: 'h2',
							name: 'foo-hunk-2',
							kind: 'hunk',
							modified: subSeconds(new Date(), 15),
							filePath: 'src/foo.py'
						}
					]
				},
				{
					id: 'f2',
					path: 'src/bar.py',
					kind: 'file',
					hunks: [
						{
							id: 'h3',
							name: 'bar-hunk-1',
							kind: 'hunk',
							modified: subHours(new Date(), 2),
							filePath: 'src/bar.py'
						}
					]
				}
			]
		},
		{
			id: 'c2',
			name: 'bugfix',
			active: true,
			kind: 'lane',
			files: [
				{
					id: 'f3',
					path: 'src/foo.py',
					kind: 'file',
					hunks: [
						{
							id: 'h4',
							name: 'foo-hunk-3',
							kind: 'hunk',
							modified: subMinutes(new Date(), 32),
							filePath: 'src/foo.py'
						}
					]
				}
			]
		},
		{
			id: 'c3',
			name: 'stashed-things',
			active: false,
			kind: 'lane',
			files: [
				{
					id: 'f4',
					path: 'src/bar.py',
					kind: 'file',
					hunks: [
						{
							id: 'h5',
							name: 'bar-hunk-2',
							kind: 'hunk',
							modified: subHours(new Date(), 1),
							filePath: 'src/bar.py'
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
		branchData: branches
	};
};
