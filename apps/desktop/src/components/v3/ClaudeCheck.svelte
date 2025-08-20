<script lang="ts">
	import { Link, Textbox, AsyncButton } from '@gitbutler/ui';

	type Props = {
		claudeExecutable: string;
		recheckedAvailability?: 'recheck-failed' | 'recheck-succeeded';
		onUpdateExecutable: (value: string) => Promise<void>;
		onCheckAvailability: () => Promise<void>;
		showInstallationGuide?: boolean;
		showTitle?: boolean;
	};

	const {
		claudeExecutable,
		recheckedAvailability,
		onUpdateExecutable,
		onCheckAvailability,
		showInstallationGuide = true,
		showTitle = true
	}: Props = $props();
</script>

<div class="claude-check">
	{#if showTitle}
		<h4 class="header-16 text-bold">Claude Code can't be found</h4>
	{/if}

	{#if showInstallationGuide}
		<p class="text-14">
			If you have yet to install Claude Code, please refer to the <Link
				target="_blank"
				href="https://docs.anthropic.com/en/docs/claude-code/quickstart"
				>Claude Code installation guide</Link
			>.
		</p>
	{/if}

	{#if showTitle}
		<p class="text-14">If you have installed Claude Code, configure the executable path below:</p>
	{/if}

	<div class="claude-config">
		<Textbox
			label="Claude executable path"
			value={claudeExecutable}
			placeholder="claude"
			onchange={onUpdateExecutable}
		/>

		{#if recheckedAvailability === 'recheck-failed'}
			<div class="claude-status claude-status--unavailable">
				✗ Claude Code not found at the specified path.
			</div>
		{:else if recheckedAvailability === 'recheck-succeeded'}
			<div class="claude-status claude-status--available">✓ Claude Code is available</div>
		{/if}

		<AsyncButton style="neutral" kind="outline" action={onCheckAvailability}>
			Test Connection
		</AsyncButton>
	</div>
</div>

<style lang="postcss">
	.claude-check {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.claude-config {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.claude-status {
		padding: 8px 12px;
		border-radius: var(--radius-m);
		font-weight: 600;
		font-size: 12px;
		text-align: center;
	}

	.claude-status--available {
		border: 1px solid var(--clr-theme-succ-border);
		background-color: var(--clr-theme-succ-bg);
		color: var(--clr-theme-succ-element);
	}

	.claude-status--unavailable {
		border: 1px solid var(--clr-theme-err-border);
		background-color: var(--clr-theme-err-bg);
		color: var(--clr-theme-err-element);
	}
</style>
