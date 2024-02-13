<script lang="ts">
	import AccountLink from '$lib/components/AccountLink.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import Link from '$lib/components/Link.svelte';
	import * as events from '$lib/utils/events';
	import type { User } from '$lib/backend/cloud';

	export let user: User | undefined;
	export let projectId: string | undefined;
</script>

<div class="footer" style:border-color="var(--clr-theme-container-outline-light)">
	<div class="left-btns">
		<IconButton
			icon="mail"
			help="Send feedback"
			on:click={() => events.emit('openSendIssueModal')}
		/>
		<Link href={`/${projectId}/settings`}>
			<IconButton icon="settings" help="Project settings" />
		</Link>
	</div>
	<AccountLink {user} />
</div>

<style lang="postcss">
	.footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-12);
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}
	.left-btns {
		display: flex;
		align-items: center;
	}
</style>
