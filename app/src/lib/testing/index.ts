import { baseBranch, project, remoteBranchData, user, virtualBranches } from './fixtures';
import { mockIPC } from '@tauri-apps/api/mocks';
import { mockWindows } from '@tauri-apps/api/mocks';

export function mockTauri() {
	mockWindows('main');
	mockIPC((cmd, args) => {
		console.log(`%c${cmd}`, 'background: #222; color: #4db2ad', args);

		// @ts-expect-error 'message' is dynamic
		if (cmd === 'tauri' && args.message?.cmd === 'openDialog') {
			return '/Users/user/project';
		}

		if (cmd === 'list_projects') {
			return [project];
		}

		if (cmd === 'get_project' && args.id === 'ac44a3bb-8bbb-4af9-b8c9-7950dd9ec295') {
			return project;
		}

		if (cmd === 'git_head') {
			return 'refs/heads/gitbutler/integration';
		}

		if (cmd === 'menu_item_set_enabled') {
			return null;
		}

		if (cmd === 'fetch_from_remotes') {
			return true;
		}

		if (cmd === 'get_base_branch_data') {
			return baseBranch;
		}

		if (cmd === 'list_virtual_branches') {
			return virtualBranches;
		}

		if (cmd === 'list_remote_branches') {
			return [];
		}

		if (cmd === 'get_remote_branch_data') {
			return remoteBranchData;
		}

		if (cmd === 'get_remote_branchs') {
			return ['refs/heads/abc123'];
		}

		if (cmd === 'get_user') {
			return user;
		}
	});
}
