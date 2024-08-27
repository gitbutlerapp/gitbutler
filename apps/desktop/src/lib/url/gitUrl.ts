import gitUrlParse from 'git-url-parse';

export type RepoInfo = {
	source: string;
	name: string;
	owner: string;
	resource: string;
	organization?: string;
	protocol?: string;
};

export function parseRemoteUrl(url: string): RepoInfo | undefined {
	try {
		const { protocol, source, name, owner, organization, resource } = gitUrlParse(url);

		return {
			protocol,
			source,
			name,
			owner,
			organization,
			resource
		};
	} catch {
		return undefined;
	}
}
