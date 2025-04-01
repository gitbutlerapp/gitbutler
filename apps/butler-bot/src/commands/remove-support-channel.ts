import type { Command } from '@/types';

export const removeSupportChannel: Command = {
	name: 'removesupportchannel',
	help: 'Removes the current channel from support channels. Usage: !removesupportchannel',
	butlerOnly: true,
	execute: async (message, prisma) => {
		try {
			// Check if the channel is registered
			const existingChannel = await prisma.supportChannel.findUnique({
				where: { channel_id: message.channel.id }
			});

			if (!existingChannel) {
				await message.reply('This channel is not registered as a support channel.');
				return;
			}

			// Remove the channel from the database
			await prisma.supportChannel.delete({
				where: { channel_id: message.channel.id }
			});

			await message.reply('✅ This channel has been removed from support channels.');
		} catch (error) {
			console.error('Error removing support channel:', error);
			await message.reply('There was an error removing this channel from support channels.');
		}
	}
} as Command;
