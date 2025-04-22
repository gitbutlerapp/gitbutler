import type { Task } from '@/types';

export const syncButlers: Task = {
	name: 'sync-butlers',
	schedule: '*/10 * * * *', // Run every 10 minutes
	execute: async ({ prisma, client }) => {
		try {
			// Get the allowed guild ID from environment variables
			const allowedGuildId = process.env.ALLOWED_GUILD;
			if (!allowedGuildId) {
				console.error('ALLOWED_GUILD not set in environment variables');
				return;
			}

			// Get the butler role ID from environment variables
			const butlerRoleId = process.env.BUTLER_ROLE;
			if (!butlerRoleId) {
				console.error('BUTLER_ROLE not set in environment variables');
				return;
			}

			// Fetch the allowed guild
			const guild = await client.guilds.fetch(allowedGuildId);
			if (!guild) {
				console.error('Could not find the allowed guild');
				return;
			}

			// We must fetch the guild members before we get the butler role.
			// If we don't do this, we won't be able to call butlerRole.members.values()
			await guild.members.fetch();

			// Get the butler role
			const butlerRole = await guild.roles.fetch(butlerRoleId);
			if (!butlerRole) {
				console.error('Could not find the butler role in the allowed guild');
				return;
			}

			// Get all members with the butler role
			const butlerUsers = Array.from(butlerRole.members.values());

			// Get current butlers from database
			const currentButlers = await prisma.butlers.findMany();
			const currentButlerIds = new Set(currentButlers.map((b) => b.discord_id));

			// Add new butlers
			for (const user of butlerUsers) {
				if (!currentButlerIds.has(user.id)) {
					await prisma.butlers.create({
						data: {
							discord_id: user.id,
							name: user.user.username,
							in_support_rota: false
						}
					});
				}
			}

			// Remove butlers that no longer have the role
			for (const butler of currentButlers) {
				if (!butlerUsers.some((user) => user.id === butler.discord_id)) {
					await prisma.butlers.delete({
						where: { id: butler.id }
					});
				}
			}

			// eslint-disable-next-line no-console
			console.log('Butler sync completed successfully');
		} catch (error) {
			console.error('Error syncing butlers:', error);
		}
	}
};
