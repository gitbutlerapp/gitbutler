<script lang="ts">
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button, Textbox } from '@gitbutler/ui';

	type Props = {
		type: 'group' | 'private' | 'server';
	} & ({ type: 'group'; channel: string } | { type: 'private'; nick: string } | { type: 'server' });

	const args: Props = $props();

	const ircService = inject(IRC_SERVICE);

	let input = $state('');

	async function onclick() {
		if (!input) return;
		if (args.type === 'group') {
			await ircService.sendToGroup(args.channel, input);
		} else if (args.type === 'private') {
			await ircService.sendToNick(args.nick, input);
		} else if (args.type === 'server') {
			ircService.send(input);
		}
		input = '';
	}
</script>

<div class="irc-input">
	<Textbox
		bind:value={input}
		wide
		onkeydown={(e) => {
			if (e.key === 'Enter') onclick();
		}}
	/>
	<Button type="button" size="button" style="pop" {onclick}>send</Button>
</div>

<style lang="postcss">
	.irc-input {
		display: flex;
		padding: 12px 14px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
