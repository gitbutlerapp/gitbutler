import type { Command } from '@/types';

export const runTask: Command = {
	name: 'runtask',
	help: 'Runs a specified task immediately. Usage: `!runtask <task-name>`',
	butlerOnly: true,
	execute: async ({ message, prisma, octokit, tasks, openai }) => {
		try {
			// Get the task name from the message
			const args = message.content.slice('!runtask'.length).trim().split(/\s+/);
			const taskName = args[0];

			if (!taskName) {
				// List available tasks if no task name is provided
				if (tasks.length === 0) {
					await message.reply('No tasks available to run.');
					return;
				}

				const taskList = tasks.map((task) => `- \`${task.name}\``).join('\n');
				await message.reply(
					`Available tasks:\n${taskList}\n\nUse \`!runtask <task-name>\` to run a task.`
				);
				return;
			}

			// Find the task
			const task = tasks.find((t) => t.name === taskName);

			if (!task) {
				await message.reply(
					`Task "${taskName}" not found. Use \`!runtask\` to see available tasks.`
				);
				return;
			}

			// Inform that the task is running
			await message.reply(`Running task "${task.name}"...`);

			// Execute the task
			await task.execute({ prisma, octokit, client: message.client, openai });

			// Confirm task completion
			await message.reply(`Task "${task.name}" completed successfully.`);
		} catch (error) {
			console.error('Error running task:', error);
			await message.reply('There was an error running the task.');
		}
	}
} as Command;
