import { Octokit } from '@octokit/rest';
import OpenAI from 'openai';
import type { PrismaClient } from '@prisma/client';
import type { Message, Client } from 'discord.js';

export type Command = {
	name: string;
	aliases?: string[];
	help: string;
	butlerOnly?: boolean;
	execute: (params: {
		message: Message;
		prisma: PrismaClient;
		octokit: Octokit;
		client: Client;
		commands: Command[];
		tasks: Task[];
		openai: OpenAI;
	}) => Promise<void>;
};

export type TaskExtra = {
	[key: string]: any;
};

export type Task = {
	name: string;
	schedule: string; // cron expression
	execute: (params: {
		prisma: PrismaClient;
		octokit: Octokit;
		client: Client;
		openai: OpenAI;
	}) => Promise<void>;
};
