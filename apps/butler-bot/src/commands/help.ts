import type { Command, CommandExtra } from '@/types';
import type { PrismaClient } from '@prisma/client';
import type { Message } from 'discord.js';

export const help: Command = {
	name: 'help',
	help: 'Lists all available commands with descriptions.',
	execute: async (message: Message, _prisma: PrismaClient, { commands }: CommandExtra) => {
		try {
			if (!commands || commands.length === 0) {
				await message.reply('No commands are available.');
				return;
			}

			const butlerCommands = commands.filter((cmd) => cmd.butlerOnly);
			const publicCommands = commands.filter((cmd) => !cmd.butlerOnly);

			let helpText = '**Available Commands**\n\n';

			if (publicCommands.length > 0) {
				helpText += '**Public Commands:**\n';
				publicCommands.forEach((cmd) => {
					helpText += `**!${cmd.name}** - ${cmd.help}.\n`;
				});
				helpText += '\n';
			}

			if (butlerCommands.length > 0) {
				helpText += '**Butler-Only Commands:**\n';
				butlerCommands.forEach((cmd) => {
					helpText += `**!${cmd.name}** - ${cmd.help}.\n`;
				});
			}

			await message.reply(helpText);
		} catch (error) {
			console.error('Error showing help:', error);
			await message.reply('There was an error processing your help request.');
		}
	}
} as Command;
