import { GitHubBranch } from './githubBranch';
import { GitHubChecksMonitor } from './githubChecksMonitor';
import { GitHubListingService } from './githubListingService';
import { GitHubPrService } from './githubPrService';
import { invoke } from '$lib/backend/ipc';
import { showToast } from '$lib/notifications/toasts';
import { Octokit } from '@octokit/rest';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHost } from '../interface/gitHost';
import type { GitHostArguments } from '../interface/types';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements GitHost {
	private baseUrl: string;
	private repo: RepoInfo;
	private baseBranch: string;
	private forkStr?: string;
	private octokit?: Octokit;
	private projectMetrics?: ProjectMetrics;

	constructor({
		repo,
		baseBranch,
		forkStr,
		octokit,
		projectMetrics
	}: GitHostArguments & {
		octokit?: Octokit;
		projectMetrics?: ProjectMetrics;
	}) {
		this.baseUrl = `https://${GITHUB_DOMAIN}/${repo.owner}/${repo.name}`;
		this.repo = repo;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.octokit = octokit;
		this.projectMetrics = projectMetrics;
	}

	listService() {
		if (!this.octokit) {
			return;
		}
		return new GitHubListingService(this.octokit, this.repo, this.projectMetrics);
	}

	prService(baseBranch: string, upstreamName: string) {
		if (!this.octokit) {
			return;
		}
		return new GitHubPrService(this.octokit, this.repo, baseBranch, upstreamName);
	}

	checksMonitor(sourceBranch: string) {
		if (!this.octokit) {
			return;
		}
		return new GitHubChecksMonitor(this.octokit, this.repo, sourceBranch);
	}

	branch(name: string) {
		if (!this.baseBranch) {
			return;
		}
		return new GitHubBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}

	async getPrTemplateContent(path: string) {
		try {
			const fileContents: string | undefined = await invoke('get_pr_template_contents', { path });
			return fileContents;
		} catch (err) {
			console.error(`Error reading pull request template at path: ${path}`, err);

			showToast({
				title: 'Failed to read pull request template',
				message: `Could not read: \`${path}\`.`,
				style: 'neutral'
			});
		}
	}

	async getAvailablePrTemplates(path: string): Promise<string[] | undefined> {
		// TODO: Find a workaround to avoid this dynamic import
		// https://github.com/sveltejs/kit/issues/905
		const { join } = await import('@tauri-apps/api/path');
		const targetPath = await join(path, '.github');

		const availableTemplates: string[] | undefined = await invoke(
			'get_available_github_pr_templates',
			{ path: targetPath }
		);

		return availableTemplates;
	}
}
