<script lang="ts">
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	type Props = {
		type: 'group' | 'private' | 'server';
	} & ({ type: 'group'; channel: string } | { type: 'private'; nick: string } | { type: 'server' });

	const args: Props = $props();

	const ircService = getContext(IrcService);

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
		gap: 8px;
		padding: 12px 14px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
