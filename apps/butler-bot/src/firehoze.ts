import Anthropic from '@anthropic-ai/sdk';
import { PrismaClient } from '@prisma/client';
import { Message, TextChannel } from 'discord.js';
import OpenAI from 'openai';
import { ChannelType } from '@/types/channel-types';
import { compareEmbeddings, createEmbedding, parseEmbedding } from '@/utils/embedding';
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
	const lastIndex = messages.length - 1;

	const prompt = `You are a helpful assistant that analyzes Discord support conversations.

Your task is to determine the nature of the next message in the chat channel.

Try to determine if the message was looking for support, responding to a previous message, starting a new conversation, or something else.

A user might not directly ask for support. They may instead make a statement about having an issue with a given feature.

If you determine that a message is a support query, respond with "REQUEST_SUPPORT" even if it is continuing a conversation or starting a new one.

Respond with a breif explanation followed by one of the following "REQUEST_SUPPORT", "CONTINUED_CONVERSATION", "NEW_CONVERSATION", or "OTHER".

Here are four messages for context:

${messages
	.slice(0, lastIndex)
	.map((msg) => `${msg.author.username}: ${msg.content}`)
	.join('\n')}


Here is the last message:

${messages[lastIndex]!.author.username}: ${messages[lastIndex]!.content}`;

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

The title should be clear and concise, focusing on the main issue or question.

Respond with the title, no other text.`;

	const summary = await askClaude(prompt, 50);
	// Clean up the response and ensure it's not too long
	return summary.trim();
}

/**
 * Find GitHub issues related to a given message based on embedding similarity
 * @param prisma PrismaClient instance
 * @param openai OpenAI instance
 * @param summarizeHelpRequest The summarized help request title
 * @param similarityThreshold Minimum similarity score (0-1) to consider an issue related
 * @param limit Maximum number of related issues to return
 * @returns Array of related issues with similarity scores
 */
async function findRelatedIssues(
	prisma: PrismaClient,
	openai: OpenAI,
	summarizeHelpRequest: string,
	similarityThreshold: number = 0.5,
	limit: number = 3
) {
	try {
		// Create embedding for the message
		const messageEmbedding = await createEmbedding(openai, summarizeHelpRequest);

		// Get all GitHub issues from the database
		const issues = await prisma.gitHubIssue.findMany({
			include: {
				repository: true
			},
			where: {
				embedding: {
					not: null
				}
			}
		});

		if (issues.length === 0) {
			return [];
		}

		// Calculate similarity scores for each issue
		const issuesWithScores = issues.map((issue) => {
			// Parse stored embedding
			const issueEmbedding = parseEmbedding(issue.embedding!);

			// Calculate similarity score
			const similarityScore = compareEmbeddings(messageEmbedding, issueEmbedding);

			return {
				issue,
				similarityScore
			};
		});

		// Filter by similarity threshold and sort by similarity score (highest first)
		let relatedIssues = issuesWithScores
			.sort((a, b) => b.similarityScore - a.similarityScore)
			.slice(0, limit);

		relatedIssues = relatedIssues.filter((issue) => issue.similarityScore >= similarityThreshold);
		if (relatedIssues.length === 0) {
			relatedIssues = [relatedIssues[0]!];
		}

		return relatedIssues.map((issue) => issue.issue);
	} catch (error) {
		console.error('Error finding related issues:', error);
		return [];
	}
}

export async function firehoze(prisma: PrismaClient, message: Message, openai: OpenAI) {
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
			const relatedIssues = await findRelatedIssues(prisma, openai, ticketName);
			const ticket = await prisma.supportTicket.create({
				data: {
					name: ticketName,
					link: message.url,
					github_issues: {
						connect: relatedIssues.map((issue) => ({ id: issue.id }))
					}
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
						if (channel && channel.isTextBased()) {
							const textChannel = channel as TextChannel;
							const alertMessage =
								`🎫 New ticket: ${formatTicket(ticket, relatedIssues)}\n` +
								`<@${onDutyButler.discord_id}>, there's a new support ticket during business hours.`;

							await textChannel.send(alertMessage);
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
