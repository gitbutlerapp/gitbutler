import type { Command } from '@/types';

export const restart: Command = {
	name: 'restart',
	help: 'Restarts the butler-bot by exiting with code 0.',
	aliases: ['reboot'],
	butlerOnly: true,
	execute: async ({ message }) => {
		try {
			await message.reply('🔄 Restarting butler-bot...');

			// Use setTimeout to ensure the message is sent before exiting
			// eslint-disable-next-line no-console
			console.log('Restart command received. Exiting with code 0 to trigger restart.');
			process.exit(0);
		} catch (error) {
			console.error('Error restarting bot:', error);
			await message.reply('There was an error while trying to restart the bot.');
		}
	}
} as Command;
