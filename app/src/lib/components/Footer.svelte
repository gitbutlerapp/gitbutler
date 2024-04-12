<script lang="ts">
	import AccountLink from '$lib/components/AccountLink.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import * as events from '$lib/utils/events';
	import { goto } from '$app/navigation';

	export let projectId: string | undefined;
	export let isNavCollapsed: boolean;
</script>

<div class="footer" class:collapsed={isNavCollapsed}>
	<div class="left-btns">
		<IconButton
			icon="mail"
			help="Send feedback"
			size="l"
			width={isNavCollapsed ? '100%' : undefined}
			on:mousedown={() => events.emit('openSendIssueModal')}
		/>
		<IconButton
			icon="settings"
			help="Project settings"
			size="l"
			width={isNavCollapsed ? '100%' : undefined}
			on:mousedown={async () => await goto(`/${projectId}/settings`)}
		/>
	</div>
	<AccountLink {isNavCollapsed} />
</div>

<style lang="postcss">
	.footer {
		display: flex;
		justify-content: space-between;
		padding: var(--size-12);
		border-top: 1px solid var(--clr-border-main);
		border-color: var(--clr-border-main);
	}

	.left-btns {
		display: flex;
		gap: var(--size-2);
	}

	.footer.collapsed {
		flex-direction: column;
		padding: 0 var(--size-14);
		align-items: flex-start;
		gap: var(--size-4);
		border: none;

		& .left-btns {
			flex-direction: column;
			width: 100%;
		}
	}
</style>
