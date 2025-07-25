import type { Command } from '@/types';

export const ping: Command = {
	name: 'ping',
	help: 'A simple command to check if the bot is responding.',
	execute: async ({ message }) => {
		await message.reply('Pong!');
	}
} as Command;
