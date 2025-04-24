import type { Command } from '@/types';

export const removeRepo: Command = {
	name: 'removerepo',
	help: 'Removes a GitHub repository from tracking. Usage: `!removerepo <owner/name>`',
	aliases: ['deleterepo'],
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			const args = message.content.split(' ');
			if (args.length < 2) {
				await message.reply(
					'Please specify either a repository ID or repository in the format `owner/name`.'
				);
				return;
			}

			const repoFullName = args[1];

			if (!repoFullName) {
				await message.reply('Please specify a repository in the format `owner/name`.');
				return;
			}

			// Validate repository format
			if (!repoFullName.includes('/')) {
				await message.reply('Please specify a repository in the format `owner/name`.');
				return;
			}

			const [owner, name] = repoFullName.split('/');

			if (!owner || !name) {
				await message.reply('Please specify a repository in the format `owner/name`.');
				return;
			}

			const repo = await prisma.gitHubRepo.findFirst({
				where: {
					owner,
					name
				}
			});

			if (!repo) {
				await message.reply('Repository not found in the database.');
				return;
			}

			// Delete the repository
			await prisma.gitHubRepo.delete({
				where: { id: repo.id }
			});

			await message.reply(`✅ Repository ${repo.owner}/${repo.name} has been removed.`);
		} catch (error) {
			console.error('Error removing repository:', error);
			await message.reply('There was an error removing this repository.');
		}
	}
} as Command;
