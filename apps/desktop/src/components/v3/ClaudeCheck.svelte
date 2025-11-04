<script lang="ts">
	import { Icon, Link, Textbox, AsyncButton } from '@gitbutler/ui';

	type Props = {
		claudeExecutable: string;
		recheckedAvailability?: 'recheck-failed' | 'recheck-succeeded';
		onUpdateExecutable: (value: string) => Promise<void>;
		onCheckAvailability: () => Promise<void>;
		showTitle?: boolean;
	};

	const {
		claudeExecutable,
		recheckedAvailability,
		onUpdateExecutable,
		onCheckAvailability,
		showTitle = true
	}: Props = $props();

	let isChecking = $state(false);
	let showResult = $state(false);

	async function handleCheckAvailability() {
		isChecking = true;
		showResult = false;
		try {
			await onCheckAvailability();
			showResult = true;
		} finally {
			isChecking = false;
			// Show the result for a few seconds before showing the button again
			setTimeout(() => {
				showResult = false;
			}, 2000);
		}
	}

	const DOCS_URL = 'https://docs.gitbutler.com/features/agents-tab#installing-claude-code';
</script>

{#if showTitle}
	<div class="flex items-center gap-8">
		{#if recheckedAvailability === 'recheck-failed'}
			<Icon name="warning" color="warning" />
			<h4 class="text-16 text-semibold text-body">Claude Code can't be found</h4>
		{:else}
			<Icon name="success" color="success" />
			<h4 class="text-16 text-semibold text-body">Claude code is connected</h4>
		{/if}
	</div>
{/if}

<div class="claude-config">
	<Textbox
		label="Claude Code path:"
		value={claudeExecutable}
		placeholder="Path to the Claude Code executable"
		onchange={onUpdateExecutable}
	/>

	{#if isChecking || showResult}
		<div
			class="claude-test-result-messaege"
			class:success={recheckedAvailability === 'recheck-succeeded'}
			class:error={recheckedAvailability === 'recheck-failed'}
		>
			{#if isChecking}
				<p class="text-12">Checking connection...</p>
				<Icon name="spinner" />
			{:else if recheckedAvailability === 'recheck-failed'}
				<p class="text-12">Couldn't connect. Check the path and try again</p>
				<Icon name="error-small" />
			{:else if recheckedAvailability === 'recheck-succeeded'}
				<p class="text-12">You're all set! Connection's good!</p>
				<Icon name="tick" />
			{/if}
		</div>
	{:else}
		<AsyncButton style="neutral" kind="outline" action={handleCheckAvailability} icon="plug">
			{#if recheckedAvailability === 'recheck-failed'}
				Connect to Claude Code
			{:else}
				Test Connection
			{/if}
		</AsyncButton>
	{/if}
</div>

<div class="claude-check">
	<Icon name="docs" color="info" />
	<p class="text-13 text-body">
		{#if recheckedAvailability === 'recheck-failed'}
			If you haven't installed Claude Code, check our <Link href={DOCS_URL}>installation guide</Link
			>
		{:else}
			Get the full guide to Agents in GitButler in <Link href={DOCS_URL}>our documentation</Link>
		{/if}
	</p>
</div>

<style lang="postcss">
	.claude-check {
		display: flex;
		align-items: center;
		padding: 10px 14px;
		gap: 14px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.claude-config {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.claude-test-result-messaege {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-button);
		padding: 0 12px;
		gap: 4px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);

		&.success {
			background-color: var(--clr-theme-succ-soft);
			color: var(--clr-theme-succ-on-soft);
		}

		&.error {
			background-color: var(--clr-theme-err-soft);
			color: var(--clr-theme-err-on-soft);
		}
	}
</style>
