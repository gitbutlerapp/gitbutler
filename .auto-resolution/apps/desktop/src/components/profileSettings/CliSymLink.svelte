<script lang="ts">
	import { BACKEND } from '$lib/backend';
	import { CLI_MANAGER } from '$lib/cli/cli';
	import { inject } from '@gitbutler/core/context';
	import { Icon } from '@gitbutler/ui';
	import { copyToClipboard } from '@gitbutler/ui/utils/clipboard';

	interface Props {
		class?: string;
	}

	const { class: classes = '' }: Props = $props();

	const cliManager = inject(CLI_MANAGER);
	const cliPath = cliManager.path();
	const backend = inject(BACKEND);
	const platformName = backend.platformName;

	function cliCommand(path: string, platform: string): string {
		if (platform === 'windows') {
			// Windows-specific instructions - copy to WindowsApps which is typically in PATH
			return `copy "${path}" "%LOCALAPPDATA%\\Microsoft\\WindowsApps\\but.exe"`;
		} else {
			// Unix-like systems (macOS, Linux)
			return "sudo ln -sf '" + path + "' /usr/local/bin/but";
		}
	}
</script>

<div class="symlink-copy-box {classes}">
	{#if cliPath.response}
		{@const command = cliCommand(cliPath.response, platformName)}
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
		font-family: var(--font-mono);
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
