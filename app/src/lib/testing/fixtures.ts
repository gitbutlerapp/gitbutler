import { pnpmLock } from './fileHunks';
import type { User } from '$lib/stores/user';

export type Constructor<T = any> = new (...args: any[]) => T;
export type Class<T = any> = InstanceType<Constructor<T>>;

export const project = {
	api: null,
	description: null,
	gitbutler_code_push_state: null,
	gitbutler_data_last_fetch: null,
	id: 'ac44a3bb-8bbb-4af9-b8c9-7950dd9ec295',
	ok_with_force_push: true,
	omit_certificate_check: null,
	path: '/opt/ndomino/home2021',
	preferred_key: 'systemExecutable',
	project_data_last_fetch: Object,
	fetched: {
		timestamp: {
			nanos_since_epoch: 410569736,
			secs_since_epoch: 1714924416
		}
	},
	title: 'home2021'
};

export const author = {
	name: 'John Snow',
	email: 'user@company.com',
	gravatarUrl: 'https://gravatar.com/avatar/abc123'
};

export const remoteCommit0 = {
	id: 'fe30876278739f7182effd27e9d9debde648b4de',
	author,
	description: 'fix: updated files',
	createdAt: 1714902366
};

export const remoteCommit1 = {
	id: 'fe30876278739f7182effd27e9d9debde648b4dd',
	author,
	description: 'fix: updated files',
	createdAt: 1714902366
};

export const remoteBranch0 = {
	sha: '90c225edcc74b31718a9cd8963c1bc89c17d8863',
	name: '',
	upstream: '',
	lastCommitTimestampMs: 1714902366140,
	lastCommitAuthor: 'John Snow'
};

export const baseBranch = {
	branchName: 'origin/gitbutler/integration',
	remoteName: 'origin',
	remoteUrl: 'ssh://github.com/user/project.git',
	baseSha: '90c225edcc74b31718a9cd8963c1bc89c17d8864',
	currentSha: '90c225edcc74b31718a9cd8963c1bc89c17d8864',
	behind: 0,
	upstreamCommits: [],
	recentCommits: [remoteCommit0],
	lastFetchedMs: 1714843209991
};

export const user: User = {
	access_token: '00000000-0000-0000-0000-000000000000',
	created_at: '2024-05-04T13:27:30Z',
	email: 'yo@ndo.dev',
	family_name: undefined,
	github_access_token: undefined,
	github_username: undefined,
	given_name: undefined,
	id: 31,
	locale: 'en_US',
	name: 'Nico',
	picture: 'https://source.boringavatar.com/marble/120',
	role: undefined,
	updated_at: '2024-05-05T15:38:02Z',
	supporter: false
};

export const remoteBranchData = {
	sha: '90c225edcc74b31718a9cd8963c1bc89c17d8864',
	name: 'test',
	upstream: 'abc123',
	authors: [author],
	displayName: 'test',
	lastCommitTs: new Date(),
	firstCommitAt: new Date(),
	ahead: 0,
	behind: 0,
	commits: [remoteCommit0],
	isMergeable: true
};

export const fileHunk2 = {
	binary: false,
	changeType: 'added',
	diff: pnpmLock,
	end: 4696,
	filePath: 'pnpm-lock.yaml',
	hash: 'dc79c984a36b2f8a29007633bde4daf4',
	id: '63-71',
	locked: false,
	lockedTo: null,
	modifiedAt: 1714829527993,
	oldStart: 0,
	start: 0
};

export const fileHunk = {
	binary: false,
	changeType: 'modified',
	diff: `
@@ -63,7 +63,7 @@
     "simple-git-hooks": "^2.11.1",
     "tailwindcss": "^3.4.3",
     "typescript": "^5.4.5",
-    "typescript-eslint": "^7.7.0"
+    "typescript-eslint": "^7.8.0"
   },
   "commitlint": {
     "extends": [
`,
	end: 70,
	filePath: 'package.json',
	hash: 'dc79c984a36b2f8a29007633bde4daf3',
	id: '63-70',
	locked: false,
	lockedTo: null,
	modifiedAt: 1714829527993,
	oldStart: 63,
	start: 63
};

export const file0 = {
	binary: false,
	conflicted: false,
	hunks: [fileHunk],
	id: 'package.json',
	large: false,
	modifiedAt: 1714829589111,
	path: 'package.json'
};

export const file1 = {
	binary: false,
	conflicted: false,
	hunks: [fileHunk2],
	id: 'pnpm-lock.yaml',
	large: false,
	modifiedAt: 1714829589111,
	path: 'pnpm-lock.yaml'
};

export const virtualBranch = {
	active: true,
	baseCurrent: true,
	commits: [],
	conflicted: false,
	files: [file0, file1],
	head: '90c225edcc74b31718a9cd8963c1bc89c17d8864',
	id: '29cdc7a7-3462-4c14-a037-0a6cdad68da3',
	name: 'Virtual branch',
	notes: '',
	order: 0,
	ownership:
		'package.json:63-70-dc79c984a36b2f8a29007633bde4daf3-1714829528116,23-58-fbf18cec4afef8aafbbc2dddef3e3391-1714829528116,79-85-c4d0a57fca736c384cde2a68009ffcb3-1714829503193',
	requiresForce: false,
	updatedAt: 1714829503190,
	upstream: null,
	upstreamName: null
};

export const virtualBranches = {
	branches: [virtualBranch],
	skippedFiles: []
};
