import { getActionCableEndpoint } from './utils';
import { isApiPatchEvent, type ApiPatchEvent } from '@gitbutler/shared/branches/types';
import { createConsumer } from '@rails/actioncable';

export interface ChatChannelSubscriptionParams {
	token: string;
	changeId: string;
	projectId: string;
	onEvent: (data: ApiPatchEvent) => void;
}

export function subscribeToChatChannel({
	token,
	changeId,
	projectId,
	onEvent: onData
}: ChatChannelSubscriptionParams): () => void {
	const actionCableEndpoint = getActionCableEndpoint(token);
	const consumer = createConsumer(actionCableEndpoint);
	consumer.subscriptions.create(
		{ channel: 'ChatChannel', change_id: changeId, project_id: projectId },
		{
			received(data: unknown) {
				if (!isApiPatchEvent(data)) return;
				onData(data);
			}
		}
	);

	return () => {
		consumer.disconnect();
	};
}
