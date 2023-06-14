import type { PageLoad } from './$types';
import { plainToInstance } from 'class-transformer';
import { Branch, File } from './types';

export const load: PageLoad = async () => {
	const testdata_file = await (
		await import('@tauri-apps/api/path')
	).resolveResource('../scripts/branch_testdata.json');
	const test_branches = JSON.parse(
		await (await import('@tauri-apps/api/fs')).readTextFile(testdata_file)
	);

	// fix dates from the test data
	test_branches.map((branch: Branch) => {
		branch.files = branch.files.map((file: File) => {
			file.hunks = file.hunks.map((hunk: any) => {
				hunk.modifiedAt = new Date(hunk.modifiedAt);
				return hunk;
			});
			return file;
		});

		return branch;
	});
	let branches = test_branches as Branch[];

	branches = plainToInstance(
		Branch,
		branches.map((column) => ({
			...column,
			files: column.files.map((file) => ({
				...file,
				hunks: file.hunks.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime())
			}))
		}))
	);

	return {
		branchData: branches
	};
};
