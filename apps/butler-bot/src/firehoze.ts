import Anthropic from '@anthropic-ai/sdk';
import { PrismaClient } from '@prisma/client/extension';
import { Message } from 'discord.js';
import { ChannelType } from '@/types/channel-types';

const anthropic = new Anthropic({
	apiKey: process.env.ANTHROPIC_API_KEY
});

type ConversationType = 'NEW_HELP_REQUEST' | 'CONTINUED_CONVERSATION';

async function askClaude(prompt: string, maxTokens: number = 100): Promise<string> {
	const response = await anthropic.messages.create({
		model: 'claude-3-haiku-20240307',
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

Your task is to determine if the last message represents a new request for help.

Look for indicators like:
- A new question or problem being presented
- A different topic being introduced
- A new user asking for help

Respond with a breif explanation followed by either "NEW_HELP_REQUEST" or "CONTINUED_CONVERSATION".

Here are two messages for context:

${messages
	.slice(0, 2)
	.map((msg) => `${msg.author.username}: ${msg.content}`)
	.join('\n')}


Here is the last message:

${messages[2]!.author.username}: ${messages[2]!.content}`;

	const analysis = await askClaude(prompt);

	if (analysis.includes('NEW_HELP_REQUEST')) {
		return 'NEW_HELP_REQUEST';
	}

	return 'CONTINUED_CONVERSATION';
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
		const messages = await message.channel.messages.fetch({ limit: 3 });
		const lastMessages = Array.from(messages.values());
		lastMessages.reverse();

		const conversationType = await analyzeConversation(lastMessages);
		console.warn('Conversation type:', conversationType);

		if (conversationType === 'NEW_HELP_REQUEST') {
			// Create a new ticket with a summarized title
			const ticketName = await summarizeHelpRequest(message);
			await prisma.supportTicket.create({
				data: {
					name: ticketName,
					link: message.url
				}
			});
		}
	} catch (error) {
		console.error('Error in firehoze:', error);
	}
}
