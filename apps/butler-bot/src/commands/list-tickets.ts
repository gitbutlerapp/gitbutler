import type { Command } from '@/types';

export const listTickets: Command = {
	name: 'listtickets',
	help: 'Lists all support tickets and their resolution status.',
	butlerOnly: true,
	execute: async (message, prisma) => {
		try {
			const tickets = await prisma.supportTicket.findMany({
				orderBy: { created_at: 'desc' }
			});

			if (tickets.length === 0) {
				await message.reply('No support tickets found.');
				return;
			}

			const formattedList = tickets
				.map((ticket) => {
					const status = ticket.resolved ? '✅ Resolved' : '❌ Open';
					return `**${ticket.name}** - ${status}\n${ticket.link}`;
				})
				.join('\n\n');

			await message.reply(`**Support Tickets**\n${formattedList}`);
		} catch (error) {
			console.error('Error listing tickets:', error);
			await message.reply('There was an error fetching the ticket list.');
		}
	}
} as Command;
