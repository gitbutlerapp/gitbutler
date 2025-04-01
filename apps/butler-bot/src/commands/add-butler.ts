import type { Command } from '@/types';

export const addButler: Command = {
	name: 'addbutler',
	help: 'Adds a user as a butler. Usage: `!addbutler [@user]` - If no user is mentioned, adds yourself.',
	butlerOnly: true,
	execute: async (message, prisma) => {
		try {
			// Use mentioned user or the message author
			const targetUser = message.mentions.users.first() || message.author;

			// Check if the butler already exists
			const existingButler = await prisma.butlers.findFirst({
				where: { discord_id: targetUser.id }
			});

			if (existingButler) {
				await message.reply(`${existingButler.name} is already a butler.`);
				return;
			}

			// Get the user's name (username or nickname in the server)
			let userName = targetUser.username;
			if (message.guild && message.guild.members.cache.has(targetUser.id)) {
				const member = message.guild.members.cache.get(targetUser.id);
				userName = member?.nickname || userName;
			}

			// Add the new butler
			await prisma.butlers.create({
				data: {
					discord_id: targetUser.id,
					name: userName,
					in_support_rota: false
				}
			});

			await message.reply(`${userName} has been added as a butler.`);
		} catch (error) {
			console.error('Error adding butler:', error);
			await message.reply('There was an error adding the butler.');
		}
	}
} as Command;
