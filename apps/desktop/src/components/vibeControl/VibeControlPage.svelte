<script lang="ts">
	import { invoke, listen } from '$lib/backend/ipc';
	import { Button, Textarea } from '@gitbutler/ui';

	type Props = {
		projectId: string;
	};
	const { projectId }: Props = $props();

	const stackId = crypto.randomUUID();

	let message = $state('');
	let events = $state<unknown[]>([]);

	$effect(() => {
		const us = listen(`project://${projectId}/claude/${stackId}/message_recieved`, (event) => {
			console.log(event);
			events.push(event);
		});
		return us;
	});

	async function sendMessage() {
		await invoke('claude_send_message', {
			projectId,
			stackId,
			message
		});
		message = '';
	}
</script>

{projectId}

<Textarea bind:value={message}></Textarea>
<Button onclick={sendMessage}>Click me!</Button>
