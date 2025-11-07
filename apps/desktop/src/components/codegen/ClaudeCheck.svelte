<script lang="ts">
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { inject } from '@gitbutler/core/context';
	import { Icon, Textbox, AsyncButton } from '@gitbutler/ui';
	import { fromStore } from 'svelte/store';

	type Props = {
		showTitle?: boolean;
	};

	const { showTitle }: Props = $props();

	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = fromStore(settingsService.appSettings);

	let claudeExecutable = $state('');
	let recheckedAvailability = $state<'recheck-failed' | 'recheck-succeeded'>();
	let isChecking = $state(false);
	let showSuccess = $state(false);
	let hideResultTimer: ReturnType<typeof setTimeout> | undefined = $state();

	// Check availability on mount to show correct initial state
	const availabilityQuery = $derived(claudeCodeService.checkAvailable(undefined));

	$effect(() => {
		if (settingsStore.current?.claude) {
			claudeExecutable = settingsStore.current.claude.executable;
		}
	});

	async function updateClaudeExecutable(value: string) {
		claudeExecutable = value;
		recheckedAvailability = undefined;
		await settingsService.updateClaude({ executable: value });
	}

	async function checkClaudeAvailability() {
		const recheck = await claudeCodeService.fetchCheckAvailable(undefined, { forceRefetch: true });
		if (recheck?.status === 'available') {
			recheckedAvailability = 'recheck-succeeded';
		} else {
			recheckedAvailability = 'recheck-failed';
		}
	}

	async function handleCheckAvailability() {
		isChecking = true;
		showSuccess = false;
		clearHideResultTimer();
		try {
			await checkClaudeAvailability();

			// Show success message if connection succeeded
			if (recheckedAvailability === 'recheck-succeeded') {
				showSuccess = true;
				// Show the result for a few seconds before showing the button again
				hideResultTimer = setTimeout(() => {
					showSuccess = false;
					hideResultTimer = undefined;
				}, 3000);
			}
		} finally {
			isChecking = false;
		}
	}

	function clearHideResultTimer() {
		if (hideResultTimer) {
			clearTimeout(hideResultTimer);
			hideResultTimer = undefined;
		}
	}

	function handleSuccessClick() {
		clearHideResultTimer();
		showSuccess = false;
	}

	// Derived state to check if Claude Code is not available
	const isClaudeNotAvailable = $derived(
		recheckedAvailability === 'recheck-failed' ||
			(recheckedAvailability === undefined &&
				availabilityQuery.response?.status === 'not_available')
	);
</script>

{#if showTitle}
	<div class="flex items-center gap-8 m-b-6">
		<div class="flex items-center gap-8 flex-1">
			{#if isClaudeNotAvailable}
				<Icon name="warning" color="warning" />
				<h4 class="text-16 text-semibold text-body">Claude Code can't be found</h4>
			{:else}
				<Icon name="success" color="success" />
				<h4 class="text-16 text-semibold text-body">Claude code is connected</h4>
			{/if}
		</div>
	</div>
{/if}

<div class="claude-config">
	<Textbox
		label="Claude Code path:"
		value={claudeExecutable}
		placeholder="Path to the Claude Code executable"
		onchange={updateClaudeExecutable}
		error={recheckedAvailability === 'recheck-failed'
			? "Couldn't connect. Check the path and try again"
			: undefined}
	/>

	{#if showSuccess}
		<div
			role="presentation"
			class="claude-test-result-messaege success"
			onclick={handleSuccessClick}
		>
			<p class="text-12">You're all set! Connection's good!</p>
			<Icon name="tick" />
		</div>
	{:else}
		<AsyncButton
			style="neutral"
			action={handleCheckAvailability}
			icon="update"
			loading={isChecking}
		>
			Check Connection
		</AsyncButton>
	{/if}
</div>

<style lang="postcss">
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
	}
</style>
