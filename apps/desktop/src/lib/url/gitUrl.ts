import { hashCode } from '@gitbutler/ui/utils/string';
import gitUrlParse from 'git-url-parse';

export type RepoInfo = {
	domain: string;
	name: string;
	owner: string;
	organization?: string;
	protocol?: string;
	hash?: string;
};

export function parseRemoteUrl(url: string): RepoInfo | undefined {
	try {
		const { protocol, name, owner, organization, resource } = gitUrlParse(url);
		const hash = hashCode(name + '|' + owner + '|' + name + '|' + organization);

		return {
			protocol,
			domain: resource,
			name,
			owner,
			organization,
			hash
		};
	} catch {
		return undefined;
	}
}
