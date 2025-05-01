import type { Command } from '@/types';
import { splitIntoMessages } from '@/utils/message-splitter';
import { formatTicket } from '@/utils/tickets';

export const listTickets: Command = {
	name: 'listtickets',
	aliases: ['tickets'],
	help: 'List all unresolved support tickets',
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			const tickets = await prisma.supportTicket.findMany({
				where: { resolved: false },
				include: { github_issues: true },
				orderBy: { created_at: 'desc' }
			});

			if (tickets.length === 0) {
				await message.reply(`No open support tickets found.`);
				return;
			}

			// Format each ticket individually
			const formattedTickets = tickets.map((ticket) => formatTicket(ticket, ticket.github_issues));

			// Use the utility function to split tickets into Discord-friendly messages
			const messages = splitIntoMessages(formattedTickets);

			// Send all messages
			for (const msg of messages) {
				await message.reply(msg);
			}
		} catch (error) {
			console.error('Error listing tickets:', error);
			await message.reply('Failed to list tickets. Please try again later.');
		}
	}
} as Command;
