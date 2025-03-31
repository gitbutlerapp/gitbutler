import { PrismaClient } from '@prisma/client';
import { Client, Events, GatewayIntentBits } from 'discord.js';
import "dotenv/config";

const prisma = new PrismaClient();
const client = new Client({
	intents: [
		GatewayIntentBits.Guilds,
		GatewayIntentBits.GuildMessages,
		GatewayIntentBits.MessageContent,
	]
});

// Event handler for when the bot is ready
client.once(Events.ClientReady, (readyClient) => {
	// eslint-disable-next-line no-console
	console.info(`Ready! Logged in as ${readyClient.user.tag}`);
});

// Event handler for incoming messages
client.on(Events.MessageCreate, async (message) => {
	// Ignore messages from bots to prevent potential infinite loops
	if (message.author.bot) return;

	// Basic command handling
	if (message.content.startsWith('!')) {
		const command = message.content.slice(1).toLowerCase();
		
		switch (command) {
			case 'ping':
				await message.reply('Pong!');
				break;
			case 'hello':
				await message.reply(`Hello ${message.author.username}!`);
				break;
			default:
				// Handle unknown commands
				await message.reply('I don\'t understand that command yet!');
		}
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
