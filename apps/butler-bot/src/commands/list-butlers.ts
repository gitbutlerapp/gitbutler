import type { Command } from '@/types';

export const listButlers: Command = {
	name: 'listbutlers',
	help: 'Lists all registered butlers and their support rota status.',
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			const butlers = await prisma.butlers.findMany({
				orderBy: { name: 'asc' }
			});

			if (butlers.length === 0) {
				await message.reply('No butlers are currently registered.');
				return;
			}

			const formattedList = butlers
				.map((butler) => {
					const status = butler.in_support_rota ? '✅ In rota' : '❌ Not in rota';
					return `**${butler.name}** - ${status}`;
				})
				.join('\n');

			await message.reply(`**Butler List**\n${formattedList}`);
		} catch (error) {
			console.error('Error listing butlers:', error);
			await message.reply('There was an error fetching the butler list.');
		}
	}
} as Command;
