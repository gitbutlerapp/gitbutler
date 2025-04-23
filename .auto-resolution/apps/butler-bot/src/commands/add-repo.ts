import type { Command } from '@/types';

export const addRepo: Command = {
	name: 'addrepo',
	help: `Adds a GitHub repository to track issues. Usage: !addrepo <owner/name>`,
	aliases: ['createrepo'],
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			const args = message.content.split(' ');
			if (args.length < 2) {
				await message.reply('Please specify a repository in the format `owner/name`.');
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

			// Check if repository already exists
			const existingRepo = await prisma.gitHubRepo.findFirst({
				where: {
					owner,
					name
				}
			});

			if (existingRepo) {
				await message.reply(`Repository ${owner}/${name} is already being tracked.`);
				return;
			}

			// Add the repository to the database
			const repo = await prisma.gitHubRepo.create({
				data: {
					owner,
					name
				}
			});

			await message.reply(`✅ Repository ${owner}/${name} has been added with ID: ${repo.id}.`);
		} catch (error) {
			console.error('Error adding repository:', error);
			await message.reply('There was an error adding this repository.');
		}
	}
} as Command;
