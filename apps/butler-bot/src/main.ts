import { PrismaClient } from '@prisma/client';
import { Client, Events, GatewayIntentBits, GuildMember } from 'discord.js';
import cron from 'node-cron';
import type { Command, Task } from '@/types';
import { addChannel } from '@/commands/add-channel';
import { help } from '@/commands/help';
import { listButlers } from '@/commands/list-butlers';
import { listChannels } from '@/commands/list-channels';
import { listTickets } from '@/commands/list-tickets';
import { ping } from '@/commands/ping';
import { removeChannel } from '@/commands/remove-channel';
import { resolveTicket } from '@/commands/resolve-ticket';
import { restart } from '@/commands/restart';
import { runTask } from '@/commands/run-task';
import { toggleRota } from '@/commands/toggle-rota';
import { firehoze } from '@/firehoze';
import { rotateDuty } from '@/tasks/rotate-duty';
import { syncButlers } from '@/tasks/sync-butlers';
import 'dotenv/config';

const prisma = new PrismaClient();
const client = new Client({
	intents: [
		GatewayIntentBits.GuildMembers,
		GatewayIntentBits.Guilds,
		GatewayIntentBits.GuildMessages,
		GatewayIntentBits.MessageContent
	]
});

function isButler(member: GuildMember) {
	return member.roles.cache.has(process.env.BUTLER_ROLE as string);
}

// Event handler for when the bot is ready
client.once(Events.ClientReady, (readyClient) => {
	// eslint-disable-next-line no-console
	console.info(`Ready! Logged in as ${readyClient.user.tag}`);

	// Schedule all tasks
	tasks.forEach((task) => {
		if (!cron.validate(task.schedule)) {
			console.error(`Invalid cron schedule for task ${task.name}: ${task.schedule}`);
			return;
		}

		cron.schedule(task.schedule, async () => {
			try {
				await task.execute(prisma, readyClient);
			} catch (error) {
				console.error(`Error executing task ${task.name}:`, error);
			}
		});
	});
});

const commands: Command[] = [
	ping,
	listButlers,
	toggleRota,
	help,
	listTickets,
	resolveTicket,
	addChannel,
	removeChannel,
	listChannels,
	runTask,
	restart
];

const tasks: Task[] = [syncButlers, rotateDuty];

// Event handler for incoming messages
client.on(Events.MessageCreate, async (message) => {
	// Ignore messages from bots to prevent potential infinite loops
	if (message.author.bot) return;

	// Basic command handling
	if (message.content.startsWith('!')) {
		const commandName = message.content.slice(1).toLowerCase();

		for (const command of commands) {
			if (
				commandName.startsWith(command.name) ||
				(command.aliases && command.aliases.some((alias) => commandName.startsWith(alias)))
			) {
				if (command.butlerOnly && message.member && !isButler(message.member)) {
					await message.reply('This command is only available to butlers.');
					return;
				}

				await command.execute(message, prisma, { commands, tasks });

				return;
			}
		}
		await message.reply("I don't understand that command yet!");
	}

	firehoze(prisma, message);
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
