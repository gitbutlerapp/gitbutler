import { type SubscriptionEvent, getActionCableEndpoint, isSubscriptionEvent } from './utils';
import { createConsumer } from '@rails/actioncable';

export interface ChatChannelSubscriptionParams {
	token: string;
	changeId: string;
	projectId: string;
	onEvent: (data: SubscriptionEvent) => void;
}

export function subscribeToChatChannel({
	token,
	changeId,
	projectId,
	onEvent: onData
}: ChatChannelSubscriptionParams) {
	function listenToChat() {
		const actionCableEndpoint = getActionCableEndpoint(token);
		const consumer = createConsumer(actionCableEndpoint);
		consumer.subscriptions.create(
			{ channel: 'ChatChannel', change_id: changeId, project_id: projectId },
			{
				received(data: unknown) {
					if (!isSubscriptionEvent(data)) return;
					onData(data);
				}
			}
		);

		return () => {
			consumer.disconnect();
		};
	}

	$effect(() => {
		const destroyConsumer = listenToChat();

		return () => {
			destroyConsumer();
		};
	});
}
