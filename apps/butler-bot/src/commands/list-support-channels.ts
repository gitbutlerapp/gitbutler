import { TextChannel } from 'discord.js';
import type { Command } from '@/types';

export const listSupportChannels: Command = {
	name: 'listsupportchannels',
	help: 'Lists all registered support channels and their status.',
	butlerOnly: true,
	execute: async (message, prisma) => {
		try {
			const channels = await prisma.supportChannel.findMany({
				orderBy: { created_at: 'desc' }
			});

			if (channels.length === 0) {
				await message.reply('No support channels are currently registered.');
				return;
			}

			const formattedList = [];
			for (const channel of channels) {
				try {
					const discordChannel = await message.client.channels.fetch(channel.channel_id);
					if (discordChannel instanceof TextChannel) {
						formattedList.push(`**${discordChannel.name}** (${channel.channel_id})`);
					} else {
						formattedList.push(`**Unknown Channel Type** (${channel.channel_id})`);
					}
				} catch (error) {
					console.error(`Error fetching channel ${channel.channel_id}:`, error);
					formattedList.push(`**Unknown Channel** (${channel.channel_id})`);
				}
			}

			await message.reply(`**Support Channels**\n${formattedList.join('\n')}`);
		} catch (error) {
			console.error('Error listing support channels:', error);
			await message.reply('There was an error fetching the support channel list.');
		}
	}
} as Command;
