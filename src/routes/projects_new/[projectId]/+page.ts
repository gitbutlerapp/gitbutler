import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';
import type { Branch } from './types';
import { subSeconds, subMinutes, subHours } from 'date-fns';

export const load: PageLoad = async () => {
	const branches: Branch[] = [
		{
			id: 'b1',
			name: 'feature-1',
			active: true,
			kind: 'branch',
			commits: [
				{
					id: 'c1',
					description: 'First commit',
					committedAt: subMinutes(new Date(), 3),
					kind: 'commit',
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
									modifiedAt: subMinutes(new Date(), 5),
									filePath: 'src/foo.py'
								},
								{
									id: 'h2',
									name: 'foo-hunk-2',
									kind: 'hunk',
									modifiedAt: subSeconds(new Date(), 15),
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
									modifiedAt: subHours(new Date(), 2),
									filePath: 'src/bar.py'
								}
							]
						}
					]
				}
			]
		},
		{
			id: 'b2',
			name: 'bugfix',
			active: true,
			kind: 'branch',
			commits: [
				{
					id: 'c2',
					description: 'Second commit',
					committedAt: subMinutes(new Date(), 10),
					kind: 'commit',
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
									modifiedAt: subMinutes(new Date(), 32),
									filePath: 'src/foo.py'
								}
							]
						}
					]
				}
			]
		},
		{
			id: 'b3',
			name: 'stashed-things',
			active: false,
			kind: 'branch',
			commits: [
				{
					id: 'c3',
					description: 'Third commit',
					committedAt: subMinutes(new Date(), 50),
					kind: 'commit',
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
									modifiedAt: subHours(new Date(), 1),
									filePath: 'src/bar.py'
								}
							]
						}
					]
				}
			]
		}
	].map((column) => ({
		...column,
		commits: column.commits.map((commit) => ({
			...commit,
			files: commit.files.map((file) => ({
				...file,
				hunks: file.hunks.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime())
			}))
		}))
	}));

	return {
		branchData: branches
	};
};
