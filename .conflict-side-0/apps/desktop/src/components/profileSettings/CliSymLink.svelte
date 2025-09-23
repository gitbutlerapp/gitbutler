<script lang="ts">
	import { CLI_MANAGER } from '$lib/cli/cli';
	import { inject } from '@gitbutler/core/context';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { Icon } from '@gitbutler/ui';

	interface Props {
		class?: string;
	}

	const { class: classes = '' }: Props = $props();

	const cliManager = inject(CLI_MANAGER);
	const cliPath = cliManager.path();

	function cliCommand(path: string): string {
		const command = "sudo ln -sf '" + path + "' /usr/local/bin/but";
		return command;
	}
</script>

<div class="symlink-copy-box {classes}">
	{#if cliPath.response}
		{@const command = cliCommand(cliPath.response)}
		<p>{command}</p>
		<button type="button" class="symlink-copy-icon" onclick={() => copyToClipboard(command)}>
			<Icon name="copy" />
		</button>
	{/if}
</div>

<style lang="postcss">
	.symlink-copy-box {
		display: flex;
		padding: 8px 10px;
		gap: 10px;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1-muted);
		color: var(--clr-text-1);
		font-size: 12px;
		font-family: var(--fontfamily-mono);
		word-break: break-all;
	}

	.symlink-copy-icon {
		display: flex;
		color: var(--clr-text-3);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
