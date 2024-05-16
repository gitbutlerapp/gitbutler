<script lang="ts">
	import Button from './Button.svelte';
	import AccountLink from '$lib/components/AccountLink.svelte';
	import * as events from '$lib/utils/events';
	import { goto } from '$app/navigation';

	export let projectId: string | undefined;
	export let isNavCollapsed: boolean;
</script>

<div class="footer" class:collapsed={isNavCollapsed}>
	<div class="left-btns">
		<Button
			icon="mail"
			style="ghost"
			size="cta"
			help="Share feedback"
			on:click={() => events.emit('openSendIssueModal')}
			wide={isNavCollapsed}
		/>
		<Button
			icon="settings"
			style="ghost"
			size="cta"
			help="Project settings"
			on:click={async () => await goto(`/${projectId}/settings`)}
			wide={isNavCollapsed}
		/>
		<Button
			icon="timeline"
			style="ghost"
			size="cta"
			help="Project history"
			on:click={() => events.emit('openHistory')}
			wide={isNavCollapsed}
		/>
	</div>
	<AccountLink {isNavCollapsed} />
</div>

<style lang="postcss">
	.footer {
		display: flex;
		justify-content: space-between;
		padding: var(--size-12);
		gap: var(--size-6);
		border-top: 1px solid var(--clr-border-2);
		border-color: var(--clr-border-2);
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
