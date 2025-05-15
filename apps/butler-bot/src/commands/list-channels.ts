import type { Command } from '@/types';
import { ChannelType } from '@/types/channel-types';
import { splitIntoMessages } from '@/utils/message-splitter';

export const listChannels: Command = {
	name: 'listchannels',
	help: 'Lists all registered channels or channels of a specific type. Usage: !listchannels [type]',
	aliases: ['channels'],
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			const args = message.content.split(' ');
			const channelType = args[1]?.toLowerCase();
			let whereClause = {};

			// If a valid type is specified, filter by that type
			if (channelType === ChannelType.SUPPORT || channelType === ChannelType.BUTLER_ALERTS) {
				whereClause = { type: channelType };
			} else if (channelType) {
				await message.reply(
					`Invalid channel type. Please use "${ChannelType.SUPPORT}", "${ChannelType.BUTLER_ALERTS}", or leave blank to list all channels.`
				);
				return;
			}

			const channels = await prisma.channel.findMany({
				where: whereClause,
				orderBy: { created_at: 'desc' }
			});

			if (channels.length === 0) {
				await message.reply(
					channelType
						? `No ${channelType} channels are currently registered.`
						: 'No channels are currently registered.'
				);
				return;
			}

			const formattedList = [];
			for (const channel of channels) {
				try {
					formattedList.push(`**<#${channel.channel_id}>** - Type: ${channel.type}`);
				} catch (error) {
					console.error(`Error fetching channel ${channel.channel_id}:`, error);
					formattedList.push(`**Unknown Channel** - Type: ${channel.type} (${channel.channel_id})`);
				}
			}

			const title = channelType
				? `**${channelType.charAt(0).toUpperCase() + channelType.slice(1)} Channels**`
				: '**All Registered Channels**';

			// Create the complete message
			const response = `${title}\n${formattedList.join('\n')}`;

			// Split the response if it's too long
			const messages = splitIntoMessages(response);

			// Send all message parts
			for (const msg of messages) {
				await message.reply(msg);
			}
		} catch (error) {
			console.error('Error listing channels:', error);
			await message.reply('There was an error fetching the channel list.');
		}
	}
} as Command;
