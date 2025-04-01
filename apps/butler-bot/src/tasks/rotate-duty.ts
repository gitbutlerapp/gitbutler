import { PrismaClient } from '@prisma/client';
import type { Task } from '@/types';
import { ChannelType } from '@/types/channel-types';

export const rotateDuty: Task = {
	name: 'rotate-duty',
	schedule: '0 9 * * 1-5', // Run at 9am on weekdays (Monday-Friday)
	execute: async (prisma: PrismaClient, client) => {
		try {
			// Get all butlers in the support rota
			const eligibleButlers = await prisma.butlers.findMany({
				where: { in_support_rota: true }
			});

			if (eligibleButlers.length === 0) {
				console.error('No butlers in the support rota to rotate.');
				return;
			}

			// Reset current on duty status for all butlers
			await prisma.butlers.updateMany({
				where: { on_duty: true },
				data: { on_duty: false }
			});

			// Randomly select the next butler
			const randomIndex = Math.floor(Math.random() * eligibleButlers.length);
			const selectedButler = eligibleButlers[randomIndex]!;

			// Set the selected butler as on duty
			await prisma.butlers.update({
				where: { id: selectedButler.id },
				data: { on_duty: true }
			});

			// Notify in the butler-alerts channel
			try {
				const alertChannel = await prisma.channel.findFirst({
					where: { type: ChannelType.BUTLER_ALERTS }
				});

				if (alertChannel) {
					const channel = await client.channels.fetch(alertChannel.channel_id);
					if (channel && 'send' in channel) {
						const message = `🔄 Butler rotation: <@${selectedButler.discord_id}> is now on support duty.`;
						await channel.send(message);
					}
				}
			} catch (channelError) {
				console.error('Error sending rotation notification:', channelError);
			}

			console.error(`Butler rotation complete: ${selectedButler.name} is now on duty.`);
		} catch (error) {
			console.error('Error rotating duty butler:', error);
		}
	}
};
