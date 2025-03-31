import type { PrismaClient } from "@prisma/client"
import type { Message } from "discord.js"

export type CommandExtra = {
	commands?: Command[];
	[key: string]: any;
};

export type Command = {
	name: string;
	help: string;
	butlerOnly?: boolean;
	execute: (message: Message, prisma: PrismaClient, extra?: CommandExtra) => Promise<void>;
}