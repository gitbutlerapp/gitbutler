import type { PageLoad } from './$types';
import { plainToInstance } from 'class-transformer';
import { Branch, File, type BranchData } from './types';
import { invoke } from '$lib/ipc';

export const load: PageLoad = async ({ params }) => {
	const branch_data = (params: { projectId: string }) =>
		invoke<Array<Branch>>('list_virtual_branches', params);

	const get_target = async (params: { projectId: string }) =>
		invoke<object>('get_target_data', params);

	const get_branches = async (params: { projectId: string }) =>
		invoke<Array<string>>('git_remote_branches', params);

	const get_branches_data = async (params: { projectId: string }) =>
		invoke<Array<BranchData>>('git_remote_branches_data', params);

	const vbranches = await branch_data({ projectId: params.projectId });
	console.log(vbranches);

	const target = await get_target({ projectId: params.projectId });
	console.log(target);

	const remote_branches = await get_branches({ projectId: params.projectId });
	console.log(remote_branches);

	const remote_branches_data = await get_branches_data({ projectId: params.projectId });
	console.log(remote_branches_data);


	//const cloud = CloudApi();

	// fix dates from the test data
	vbranches.map((branch: Branch) => {
		branch.files = branch.files.map((file: File) => {
			file.hunks = file.hunks.map((hunk: any) => {
				hunk.modifiedAt = new Date(hunk.modifiedAt);
				return hunk;
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

	// sort remote_branches_data by date
	remote_branches_data.sort((a, b) => b.lastCommitTs - a.lastCommitTs);

	return {
		branchData: branches,
		projectId: params.projectId,
		target,
		remote_branches,
		remote_branches_data
	};
};
