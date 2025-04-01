import type { Command } from '@/types';

export const addSupportChannel: Command = {
	name: 'addsupportchannel',
	help: 'Adds the current channel as a support channel. Usage: !addsupportchannel',
	butlerOnly: true,
	execute: async (message, prisma) => {
		try {
			// Check if the channel is already registered
			const existingChannel = await prisma.supportChannel.findUnique({
				where: { channel_id: message.channel.id }
			});

			if (existingChannel) {
				await message.reply('This channel is already registered as a support channel.');
				return;
			}

			// Add the channel to the database
			await prisma.supportChannel.create({
				data: {
					channel_id: message.channel.id
				}
			});

			await message.reply('✅ This channel has been registered as a support channel.');
		} catch (error) {
			console.error('Error adding support channel:', error);
			await message.reply('There was an error registering this channel as a support channel.');
		}
	}
} as Command;
