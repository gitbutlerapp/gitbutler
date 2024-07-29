import gitUrlParse from 'git-url-parse';

export type RepoInfo = {
	source: string;
	name: string;
	owner: string;
	organization?: string;
};

export function parseRemoteUrl(url: string): RepoInfo {
	const { source, name, owner, organization } = gitUrlParse(url);
	return {
		source,
		name,
		owner,
		organization
	};
}
