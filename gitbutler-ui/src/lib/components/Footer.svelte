<script lang="ts">
	import AccountLink from '$lib/components/AccountLink.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import Link from '$lib/components/Link.svelte';
	import * as events from '$lib/utils/events';
	import type { User } from '$lib/backend/cloud';

	export let user: User | undefined;
	export let projectId: string | undefined;
	export let isNavCollapsed: boolean;
</script>

<div class="footer" class:collapsed={isNavCollapsed}>
	<div class="left-btns">
		<IconButton
			icon="mail"
			help="Send feedback"
			size={isNavCollapsed ? 'xl' : 'l'}
			width={isNavCollapsed ? '100%' : undefined}
			on:click={() => events.emit('openSendIssueModal')}
		/>
		<Link href={`/${projectId}/settings`}>
			<IconButton
				icon="settings"
				help="Project settings"
				size={isNavCollapsed ? 'xl' : 'l'}
				width={isNavCollapsed ? '100%' : undefined}
			/>
		</Link>
	</div>
	<AccountLink {user} {isNavCollapsed} />
</div>

<style lang="postcss">
	.footer {
		display: flex;
		justify-content: space-between;
		padding: var(--space-12);
		border-top: 1px solid var(--clr-theme-container-outline-light);
		border-color: var(--clr-theme-container-outline-light);
	}

	.left-btns {
		display: flex;
		gap: var(--space-2);
	}

	.footer.collapsed {
		flex-direction: column;
		padding: 0 var(--space-14);
		align-items: flex-start;
		gap: var(--space-4);
		border: none;

		& .left-btns {
			flex-direction: column;
		}
	}
</style>
