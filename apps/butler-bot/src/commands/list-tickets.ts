import type { Command } from '@/types';

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

			if (tickets.length === 0) {
				await message.reply('No open support tickets found.');
				return;
			}

			const ticketList = tickets
				.map((ticket) => `**#${ticket.id}** - ${ticket.name} - ${ticket.link}`)
				.join('\n\n');

			await message.reply(`Here are all open support tickets:\n${ticketList}`);
		} catch (error) {
			console.error('Error listing tickets:', error);
			await message.reply('Failed to list tickets. Please try again later.');
		}
	}
} as Command;
