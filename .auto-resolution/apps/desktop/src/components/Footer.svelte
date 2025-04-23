<script lang="ts">
	import AccountLink from '$components/AccountLink.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import * as events from '$lib/utils/events';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	interface Props {
		projectId: string | undefined;
		isNavCollapsed: boolean;
	}

	const { projectId, isNavCollapsed }: Props = $props();

	const modeService = getContext(ModeService);
	const mode = modeService.mode;
</script>

<div class="footer" class:collapsed={isNavCollapsed}>
	<div class="left-btns">
		<Button
			icon="mail"
			kind="ghost"
			size="cta"
			tooltip="Share feedback"
			tooltipAlign="start"
			tooltipPosition={isNavCollapsed ? 'bottom' : 'top'}
			onclick={() => events.emit('openSendIssueModal')}
			wide={isNavCollapsed}
		/>
		<Button
			icon="settings"
			kind="ghost"
			size="cta"
			tooltip="Project settings"
			tooltipAlign={isNavCollapsed ? 'start' : 'center'}
			tooltipPosition={isNavCollapsed ? 'bottom' : 'top'}
			onclick={async () => await goto(`/${projectId}/settings`)}
			wide={isNavCollapsed}
			disabled={$mode?.type !== 'OpenWorkspace'}
		/>
		<Button
			icon="timeline"
			kind="ghost"
			size="cta"
			tooltip="Project history"
			tooltipAlign={isNavCollapsed ? 'start' : 'center'}
			tooltipPosition={isNavCollapsed ? 'bottom' : 'top'}
			onclick={() => events.emit('openHistory')}
			wide={isNavCollapsed}
		/>
	</div>
	<AccountLink {isNavCollapsed} />
</div>

<style lang="postcss">
	.footer {
		display: flex;
		justify-content: space-between;
		padding: 12px;
		gap: 6px;
		border-top: 1px solid var(--clr-border-2);
		border-color: var(--clr-border-2);
	}

	.left-btns {
		display: flex;
		gap: 2px;
	}

	.footer.collapsed {
		flex-direction: column;
		padding: 0 14px;
		align-items: flex-start;
		gap: 4px;
		border: none;

		& .left-btns {
			flex-direction: column;
			width: 100%;
		}
	}
</style>
