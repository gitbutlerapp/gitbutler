import type { Command } from '@/types';

export const removeChannel: Command = {
	name: 'removechannel',
	help: "Removes the current channel from the bot's registry.",
	aliases: ['unregisterchannel'],
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			// Check if the channel is registered
			const existingChannel = await prisma.channel.findUnique({
				where: { channel_id: message.channel.id }
			});

			if (!existingChannel) {
				await message.reply('This channel is not registered.');
				return;
			}

			const channelType = existingChannel.type;

			// Remove the channel from the database
			await prisma.channel.delete({
				where: { channel_id: message.channel.id }
			});

			await message.reply(`✅ This channel has been removed from ${channelType} channels.`);
		} catch (error) {
			console.error('Error removing channel:', error);
			await message.reply('There was an error removing this channel.');
		}
	}
} as Command;
