import { addButler } from '@/commands/add-butler';
import { help } from '@/commands/help';
import { listButlers } from '@/commands/list-butlers';
import { ping } from '@/commands/ping';
import { removeButler } from '@/commands/remove-butler';
import { toggleRota } from '@/commands/toggle-rota';
import { PrismaClient } from '@prisma/client';
import { Client, Events, GatewayIntentBits, GuildMember } from 'discord.js';
import "dotenv/config";
import type { Command } from '@/types';

const prisma = new PrismaClient();
const client = new Client({
	intents: [
		GatewayIntentBits.Guilds,
		GatewayIntentBits.GuildMessages,
		GatewayIntentBits.MessageContent,
	]
});


function isButler(member: GuildMember) {
	return member.roles.cache.has(process.env.BUTLER_ROLE as string);
}

// Event handler for when the bot is ready
client.once(Events.ClientReady, (readyClient) => {
	// eslint-disable-next-line no-console
	console.info(`Ready! Logged in as ${readyClient.user.tag}`);
});

const commands: Command[] = [
	ping,
	listButlers,
	addButler,
	removeButler,
	toggleRota,
	help,
];

// Event handler for incoming messages
client.on(Events.MessageCreate, async (message) => {
	// Ignore messages from bots to prevent potential infinite loops
	if (message.author.bot) return;

	// Basic command handling
	if (message.content.startsWith('!')) {
		const commandName = message.content.slice(1).toLowerCase();

		for (const command of commands) {
			if (commandName.startsWith(command.name)) {
				if (command.butlerOnly && message.member && !isButler(message.member)) {
					await message.reply('This command is only available to butlers.');
					return;
				}
				
				await command.execute(message, prisma, { commands });

				return;
			}
		}
		await message.reply('I don\'t understand that command yet!');
	}
});

async function main() {
	// Login to Discord with your client's token
	const token = process.env.DISCORD_TOKEN;
	if (!token) {
		throw new Error('DISCORD_TOKEN is not set in environment variables');
	}
	await client.login(token);
}

main()
	.then(async () => {
		await prisma.$disconnect();
	})
	.catch(async (e) => {
		console.error(e);
		await prisma.$disconnect();
		process.exit(1);
	});
