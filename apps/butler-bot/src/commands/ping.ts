import type { Command } from '@/types';

export const ping: Command = {
	name: 'ping',
	execute: async (message) => {
		await message.reply('Pong!');
	},
} as Command;