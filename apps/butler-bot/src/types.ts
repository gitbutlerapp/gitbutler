import type { PrismaClient } from '@prisma/client';
import type { Message, Client } from 'discord.js';

export type CommandExtra = {
	commands?: Command[];
	[key: string]: any;
};

export type Command = {
	name: string;
	help: string;
	butlerOnly?: boolean;
	execute: (message: Message, prisma: PrismaClient, extra?: CommandExtra) => Promise<void>;
};

export type TaskExtra = {
	[key: string]: any;
};

export type Task = {
	name: string;
	schedule: string; // cron expression
	execute: (prisma: PrismaClient, client: Client, extra?: TaskExtra) => Promise<void>;
};
