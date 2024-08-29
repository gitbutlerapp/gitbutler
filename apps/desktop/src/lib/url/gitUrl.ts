import gitUrlParse from 'git-url-parse';

export type RepoInfo = {
	domain: string;
	name: string;
	owner: string;
	organization?: string;
	protocol?: string;
};

export function parseRemoteUrl(url: string): RepoInfo | undefined {
	try {
		const { protocol, name, owner, organization, resource } = gitUrlParse(url);

		return {
			protocol,
			domain: resource,
			name,
			owner,
			organization
		};
	} catch {
		return undefined;
	}
}
