import gitUrlParse from 'git-url-parse';

export type RepoInfo = {
	source: string;
	name: string;
	owner?: string;
	organization?: string;
	protocol?: string;
};

export function parseRemoteUrl(url: string): RepoInfo {
	try {
		const { protocol, source, name, owner, organization } = gitUrlParse(url);

		return {
			protocol,
			source,
			name,
			owner,
			organization
		};
	} catch (e) {
		return {
			protocol: 'path',
			source: 'local',
			name: url
		};
	}
}
