import { GitHubIssue, SupportTicket } from '@prisma/client';

/**
 * Formats a single ticket as a string
 * @param ticket The ticket to format
 * @returns Formatted ticket string
 */
export function formatTicket(ticket: SupportTicket, issues?: GitHubIssue[]): string {
	let output = `**#${ticket.id}** - ${ticket.name} - ${ticket.link}`;
	if (issues && issues.length > 0) {
		output += `\nRelated issues:`;
		for (const issue of issues) {
			output += `\n- ${issue.title} [${issue.issue_number}](<${issue.url}>)`;
		}
	}
	return output;
}
