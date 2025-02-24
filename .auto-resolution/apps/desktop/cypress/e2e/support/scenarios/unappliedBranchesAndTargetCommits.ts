import MockBackend from '../mock/backend';
import { createMockBranchListing } from '../mock/branches';
import { createMockCommit } from '../mock/stacks';

const UNAPPLIED_BRANCH_NAMES = [
	'unapplied-branch-1',
	'unapplied-branch-2',
	'unapplied-branch-3',
	'unapplied-branch-4',
	'unapplied-branch-5',
	'unapplied-branch-6'
];

const TARGET_COMMIT_MESSAGES = [
	'Target commit message 1',
	'Target commit message 2',
	'Target commit message 3',
	'Target commit message 4',
	'Target commit message 5',
	'Target commit message 6'
];

/**
 * In this scenario, there are some unapplied branches and target commits.
 *
 * It's well suited for testing the branches page.
 */
export default class UnappliedBranchesAndTargetCommits extends MockBackend {
	constructor() {
		super();

		this.branchListings = UNAPPLIED_BRANCH_NAMES.map((branchName) =>
			createMockBranchListing({ name: branchName })
		);
		this.branchListing = this.branchListings[0]!;

		this.baseBranchCommits = TARGET_COMMIT_MESSAGES.map((message, index) =>
			createMockCommit({
				id: `target-commit-${index}`,
				message,
				createdAt: BigInt(Date.now() - index * 1000)
			})
		);
	}

	getMockPr() {
		return {
			url: 'https://api.github.com/repos/octocat/Hello-World/pulls/1347',
			id: 1,
			html_url: 'https://github.com/octocat/Hello-World/pull/1347',
			number: 42,
			state: 'open',
			locked: true,
			title: 'Integs and other meta-references',
			user: {
				login: 'estib',
				avatar_url: 'https://avatars.githubusercontent.com/u/35891811?v=4',
				type: 'User'
			},
			body: 'Please pull these awesome changes in!',
			labels: [],
			created_at: new Date().toISOString(),
			updated_at: new Date().toISOString(),
			head: {
				label: 'octocat:new-topic',
				ref: 'new-topic',
				sha: '6dcb09b5b57875f334f61aebed695e2e4193db5e',
				repo: {
					id: 1296269,
					node_id: 'MDEwOlJlcG9zaXRvcnkxMjk2MjY5',
					name: 'Hello-World',
					full_name: 'octocat/Hello-World',
					owner: {
						login: 'octocat'
					},
					fork: false,
					ssh_url: 'git@github.com:octocat/Hello-World.git',
					clone_url: 'https://github.com/octocat/Hello-World.git'
				}
			},
			base: {
				ref: 'master',
				repo: {
					git_url: 'git://github.com/octocat/Hello-World.git'
				}
			},
			draft: false
		};
	}

	getMockPRListings() {
		return [this.getMockPr()];
	}
}
