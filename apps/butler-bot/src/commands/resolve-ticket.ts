import type { Command } from '@/types';

export const resolveTicket: Command = {
	name: 'resolveticket',
	help: 'Marks a support ticket as resolved. Usage: `!resolveticket <ticket_id>`',
	aliases: ['resolve'],
	butlerOnly: true,
	execute: async ({ message, prisma }) => {
		try {
			const args = message.content.split(' ');
			if (args.length !== 2) {
				await message.reply('Please provide a ticket ID. Usage: `!resolveticket <ticket_id>`');
				return;
			}

			const ticketId = parseInt(args[1] || '0', 10);
			if (isNaN(ticketId)) {
				await message.reply('Please provide a valid ticket ID (number).');
				return;
			}

			const ticket = await prisma.supportTicket.findUnique({
				where: { id: ticketId }
			});

			if (!ticket) {
				await message.reply('Ticket not found.');
				return;
			}

			await prisma.supportTicket.update({
				where: { id: ticketId },
				data: { resolved: true }
			});

			await message.reply(`✅ Ticket "${ticket.name}" has been marked as resolved.`);
		} catch (error) {
			console.error('Error resolving ticket:', error);
			await message.reply('There was an error resolving the ticket.');
		}
	}
} as Command;
