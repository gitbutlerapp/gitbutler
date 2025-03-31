import type { Command } from '@/types';

export const removeButler: Command = {
	name: 'removebutler',
	butlerOnly: true,
	execute: async (message, prisma) => {
		try {
			// Use mentioned user or the message author
			const targetUser = message.mentions.users.first() || message.author;
			
			// Check if the butler exists
			const existingButler = await prisma.butlers.findFirst({
				where: { discord_id: targetUser.id }
			});
			
			if (!existingButler) {
				// Use username for the message if not found in database
				const name = targetUser.username;
				await message.reply(`${name} is not a butler.`);
				return;
			}
			
			// Store the name before deletion for the response message
			const butlerName = existingButler.name;
			
			// Remove the butler
			await prisma.butlers.delete({
				where: { id: existingButler.id }
			});
			
			await message.reply(`${butlerName} has been removed from butler role.`);
		} catch (error) {
			console.error('Error removing butler:', error);
			await message.reply('There was an error removing the butler.');
		}
	},
} as Command; 