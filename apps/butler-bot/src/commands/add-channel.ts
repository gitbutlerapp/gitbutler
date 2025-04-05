import type { Command } from '@/types';
import { ChannelType } from '@/types/channel-types';

export const addChannel: Command = {
	name: 'addchannel',
	help: `Adds the current channel to the specified type. Usage: !addchannel <type>`,
	aliases: ['registerchannel'],
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			const args = message.content.split(' ');
			if (args.length < 2) {
				await message.reply(
					`Please specify a channel type (${ChannelType.SUPPORT} or ${ChannelType.BUTLER_ALERTS}).`
				);
				return;
			}

			const channelTypeInput = args[1]?.toLowerCase();
			if (
				!channelTypeInput ||
				(channelTypeInput !== ChannelType.SUPPORT && channelTypeInput !== ChannelType.BUTLER_ALERTS)
			) {
				await message.reply(
					`Please specify a channel type (${ChannelType.SUPPORT} or ${ChannelType.BUTLER_ALERTS}).`
				);
				return;
			}

			// Now channelTypeInput is validated as a ChannelType
			const channelType = channelTypeInput as ChannelType;

			// Check if the channel is already registered
			const existingChannel = await prisma.channel.findUnique({
				where: { channel_id: message.channel.id }
			});

			if (existingChannel) {
				if (existingChannel.type === channelType) {
					await message.reply(`This channel is already registered as a ${channelType} channel.`);
				} else {
					await message.reply(
						`This channel is already registered as a ${existingChannel.type} channel. Please remove it first.`
					);
				}
				return;
			}

			// Add the channel to the database
			await prisma.channel.create({
				data: {
					channel_id: message.channel.id,
					type: channelType
				}
			});

			await message.reply(`✅ This channel has been registered as a ${channelType} channel.`);
		} catch (error) {
			console.error('Error adding channel:', error);
			await message.reply('There was an error registering this channel.');
		}
	}
} as Command;
