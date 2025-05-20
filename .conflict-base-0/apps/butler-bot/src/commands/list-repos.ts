import type { Command } from '@/types';
import { splitIntoMessages } from '@/utils/message-splitter';

export const listRepos: Command = {
	name: 'listrepos',
	help: 'Lists all tracked GitHub repositories and their issue counts.',
	aliases: ['repos', 'repositories'],
	butlerOnly: false,
	execute: async ({ message, prisma }) => {
		try {
			// Fetch all repositories with issue counts
			const repos = await prisma.gitHubRepo.findMany({
				include: {
					_count: {
						select: {
							issues: true
						}
					}
				},
				orderBy: {
					owner: 'asc'
				}
			});

			if (repos.length === 0) {
				await message.reply('No repositories are currently being tracked.');
				return;
			}

			// Format the response
			let response = '**Tracked GitHub Repositories:**\n\n';

			for (const repo of repos) {
				response += `- **${repo.owner}/${repo.name}** - ${repo._count.issues} issue(s)\n`;
			}

			// Split the response if it's too long
			const messages = splitIntoMessages(response);

			// Send all message parts
			for (const msg of messages) {
				await message.reply(msg);
			}
		} catch (error) {
			console.error('Error listing repositories:', error);
			await message.reply('There was an error listing the repositories.');
		}
	}
} as Command;
