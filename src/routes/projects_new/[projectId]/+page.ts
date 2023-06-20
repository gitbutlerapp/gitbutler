import type { PageLoad } from './$types';
import { plainToInstance } from 'class-transformer';
import { Branch, File } from './types';
import { invoke } from '$lib/ipc';
import { CloudApi } from '$lib/api';

export const load: PageLoad = async ({ params }) => {
	const branch_data = (params: { projectId: string }) =>
		invoke<Array<Branch>>('list_virtual_branches', params);

	const get_target = async (params: { projectId: string }) =>
		invoke<object>('get_target_data', params);

	const get_branches = async (params: { projectId: string }) =>
		invoke<Array<string>>('git_remote_branches', params);

	const vbranches = await branch_data({ projectId: params.projectId });
	console.log(vbranches);

	const target = await get_target({ projectId: params.projectId });
	console.log(target);

	const remote_branches = await get_branches({ projectId: params.projectId });
	console.log(remote_branches);

	//const cloud = CloudApi();

	// fix dates from the test data
	vbranches.map((branch: Branch) => {
		branch.files = branch.files.map((file: File) => {
			file.hunks = file.hunks
				.map((hunk: any) => {
					hunk.modifiedAt = new Date(hunk.modifiedAt);
					return hunk;
				})
				.filter((hunk: any) => {
					// only accept the hunk if hunk.diff does not contain the string '@@'
					return hunk.diff.includes('@@');
				});
			return file;
		});

		return branch;
	});
	let branches = vbranches as Branch[];

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
		branchData: branches,
		projectId: params.projectId,
		target,
		remote_branches
	};
};
