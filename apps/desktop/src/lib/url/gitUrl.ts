import gitUrlParse from 'git-url-parse';

export type RepoInfo = {
	source: string;
	name: string;
	owner: string;
};

export function parseRemoteUrl(url: string): RepoInfo {
	const { source, name, owner } = gitUrlParse(url);
	return {
		source,
		name,
		owner
	};
}
