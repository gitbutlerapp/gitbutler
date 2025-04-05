import { SupportTicket } from '@prisma/client';

/**
 * Formats a single ticket as a string
 * @param ticket The ticket to format
 * @returns Formatted ticket string
 */
export function formatTicket(ticket: SupportTicket): string {
	return `**#${ticket.id}** - ${ticket.name} - ${ticket.link}`;
}

/**
 * Formats a list of tickets
 * @param tickets Array of tickets to format
 * @param emptyMessage Message to show when no tickets are found
 * @returns Formatted string with all tickets
 */
export function formatTicketList(
	tickets: SupportTicket[],
	emptyMessage: string = 'No tickets found.'
): string {
	if (tickets.length === 0) {
		return emptyMessage;
	}

	return tickets.map((ticket) => formatTicket(ticket)).join('\n');
}
