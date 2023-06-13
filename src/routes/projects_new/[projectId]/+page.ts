import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';
import type { Branch, File } from './types';
import { subSeconds, subMinutes, subHours } from 'date-fns';
import { resolveResource } from '@tauri-apps/api/path';
import { readTextFile } from '@tauri-apps/api/fs';

export const load: PageLoad = async () => {
	const testdata = await resolveResource('../scripts/branch_testdata.json');
	const test_branches = JSON.parse(await readTextFile(testdata));

	// fix dates from the test data
	test_branches.map((branch: Branch) =>
		branch.commits.map((commit: any) => {
			commit.committedAt = new Date(commit.committedAt);
			commit.files = commit.files.map((file: File) => {
				file.hunks = file.hunks.map((hunk: any) => {
					hunk.modifiedAt = new Date(hunk.modifiedAt);
					return hunk;
				});
				return file;
			});

			return commit;
		})
	);
	let branches = test_branches as Branch[];

	branches = branches.map((column) => ({
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
