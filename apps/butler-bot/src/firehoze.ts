import Anthropic from '@anthropic-ai/sdk';
import { PrismaClient } from '@prisma/client';
import { Message } from 'discord.js';
import { ChannelType } from '@/types/channel-types';
import { formatTicket } from '@/utils/tickets';
import { isBusinessHours } from '@/utils/time';

const anthropic = new Anthropic({
	apiKey: process.env.ANTHROPIC_API_KEY
});

type ConversationType = 'REQUEST_SUPPORT' | 'CONTINUED_CONVERSATION' | 'NEW_CONVERSATION' | 'OTHER';

async function askClaude(prompt: string, maxTokens: number = 100): Promise<string> {
	// This does use the largest model, but I've done the calculations and it should be less than 0.0003 USD per message
	const response = await anthropic.messages.create({
		model: 'claude-3-7-sonnet-20250219',
		max_tokens: maxTokens,
		messages: [
			{
				role: 'user',
				content: prompt
			}
		]
	});

	return response.content[0]?.type === 'text' ? response.content[0].text : '';
}

async function analyzeConversation(messages: Message[]): Promise<ConversationType> {
	const prompt = `You are a helpful assistant that analyzes Discord support conversations.

Your task is to determine the nature of the next message in the chat channel.

Try to determine if the message was looking for support, responding to a previous message, starting a new conversation, or something else.

Respond with a breif explanation followed by one of the following "REQUEST_SUPPORT", "CONTINUED_CONVERSATION", "NEW_CONVERSATION", or "OTHER".

Here are four messages for context:

${messages
	.slice(0, 5)
	.map((msg) => `${msg.author.username}: ${msg.content}`)
	.join('\n')}


Here is the last message:

${messages[5]!.author.username}: ${messages[5]!.content}`;

	const analysis = await askClaude(prompt);

	if (analysis.includes('REQUEST_SUPPORT')) {
		return 'REQUEST_SUPPORT';
	}

	if (analysis.includes('CONTINUED_CONVERSATION')) {
		return 'CONTINUED_CONVERSATION';
	}

	if (analysis.includes('NEW_CONVERSATION')) {
		return 'NEW_CONVERSATION';
	}

	return 'OTHER';
}

async function summarizeHelpRequest(message: Message): Promise<string> {
	const prompt = `Summarize this help request into a short, descriptive title (max 60 characters):

${message.content}

The title should be clear and concise, focusing on the main issue or question.`;

	const summary = await askClaude(prompt, 50);
	// Clean up the response and ensure it's not too long
	return summary.slice(0, 60).trim();
}

export async function firehoze(prisma: PrismaClient, message: Message) {
	try {
		// Check if the current channel is a support channel
		const channel = await prisma.channel.findUnique({
			where: { channel_id: message.channel.id }
		});

		if (!channel || channel.type !== ChannelType.SUPPORT) {
			return;
		}

		// Get the last three messages from the channel
		const messages = await message.channel.messages.fetch({ limit: 5 });
		const lastMessages = Array.from(messages.values());
		lastMessages.reverse();

		const conversationType = await analyzeConversation(lastMessages);
		console.warn('Conversation type:', conversationType);

		if (conversationType === 'REQUEST_SUPPORT') {
			// Create a new ticket with a summarized title
			const ticketName = await summarizeHelpRequest(message);
			const ticket = await prisma.supportTicket.create({
				data: {
					name: ticketName,
					link: message.url
				}
			});

			// Check if we're in business hours - notify butler on duty
			if (isBusinessHours()) {
				try {
					// Find the butler on duty
					const onDutyButler = await prisma.butlers.findFirst({
						where: { on_duty: true }
					});

					// Find the butler alerts channel
					const alertChannel = await prisma.channel.findFirst({
						where: { type: ChannelType.BUTLER_ALERTS }
					});

					// Notify in butler alerts channel if both exist
					if (onDutyButler && alertChannel) {
						const channel = await message.client.channels.fetch(alertChannel.channel_id);
						if (channel && 'send' in channel) {
							const alertMessage =
								`🎫 New ticket: ${formatTicket(ticket)}\n` +
								`<@${onDutyButler.discord_id}>, there's a new support ticket during business hours.`;

							await channel.send(alertMessage);
						}
					}
				} catch (notifyError) {
					console.error('Error notifying butler on duty:', notifyError);
				}
			}
		}
	} catch (error) {
		console.error('Error in firehoze:', error);
	}
}
