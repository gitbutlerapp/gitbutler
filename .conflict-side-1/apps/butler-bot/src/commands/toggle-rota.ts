import type { Command } from '@/types';

export const toggleRota: Command = {
	name: 'togglerota',
	help: "Toggles a butler's support rota status. Usage: `!togglerota [@user]` - If no user is mentioned, toggles your own status.",
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
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

			// Toggle the rota status
			const updatedButler = await prisma.butlers.update({
				where: { id: existingButler.id },
				data: { in_support_rota: !existingButler.in_support_rota }
			});

			const newStatus = updatedButler.in_support_rota ? 'in' : 'out of';
			await message.reply(`${updatedButler.name} is now ${newStatus} the support rota.`);
		} catch (error) {
			console.error('Error toggling rota status:', error);
			await message.reply('There was an error updating the rota status.');
		}
	}
} as Command;
