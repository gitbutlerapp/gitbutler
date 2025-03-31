import type { PrismaClient } from "@prisma/client"
import type { Message } from "discord.js"

export type Command = {
	name: string;
	butlerOnly?: boolean;
	execute: (message: Message, prisma: PrismaClient) => Promise<void>;
}