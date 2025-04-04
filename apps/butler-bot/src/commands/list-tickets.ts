import type { Command } from '@/types';
import { formatTicketList } from '@/utils/tickets';

export const listTickets: Command = {
	name: 'listtickets',
	aliases: ['tickets'],
	help: 'List all unresolved support tickets',
	butlerOnly: true,
	execute: async (message, prisma) => {
		try {
			const tickets = await prisma.supportTicket.findMany({
				where: { resolved: false },
				orderBy: { created_at: 'desc' }
			});

			const formattedList = formatTicketList(tickets, 'No open support tickets found.');

			if (tickets.length > 0) {
				await message.reply(`Here are all open support tickets:\n${formattedList}`);
			} else {
				await message.reply(formattedList);
			}
		} catch (error) {
			console.error('Error listing tickets:', error);
			await message.reply('Failed to list tickets. Please try again later.');
		}
	}
} as Command;
