import type { PageLoad } from './$types';
import { plainToInstance } from 'class-transformer';
import { Branch, File } from './types';
import { invoke } from '@tauri-apps/api';
export const load: PageLoad = async ({ params }) => {
	const branch_data = (params: { projectId: string }) =>
	invoke<Array<Branch>>('list_virtual_branches', { projectId: params.projectId });

	const test_branches = await (
		branch_data({ projectId: params.projectId })
	);

	// fix dates from the test data
	test_branches.map((branch: Branch) => {
		branch.files = branch.files.map((file: File) => {
			file.hunks = file.hunks.map((hunk: any) => {
				hunk.modifiedAt = new Date(hunk.modifiedAt);
				return hunk;
			}).filter((hunk: any) => {
				// only accept the hunk if hunk.diff does not contain the string '@@'
				return hunk.diff.includes('@@');
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
