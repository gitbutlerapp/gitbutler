<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { dismissToast, toastStore } from '$lib/notifications/toasts';
	import { Markdown, TestId } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';
</script>

<div class="toast-controller hide-native-scrollbar">
	{#each $toastStore as toast (toast.id)}
		<!-- eslint-disable-next-line func-style -->
		{@const dismiss = () => dismissToast(toast.id)}
		<div transition:slide={{ duration: 170 }}>
			<InfoMessage
				testId={toast.testId ?? TestId.ToastInfoMessage}
				style={toast.style ?? 'neutral'}
				error={toast.error}
				secondaryLabel={toast.extraAction ? toast.extraAction.label : 'Dismiss'}
				secondaryTestId={toast.extraAction ? toast.extraAction.testId : undefined}
				secondaryAction={toast.extraAction ? () => toast.extraAction?.onClick(dismiss) : dismiss}
				tertiaryLabel={toast.extraAction ? 'Dismiss' : undefined}
				tertiaryAction={toast.extraAction ? dismiss : undefined}
				shadow
			>
				{#snippet title()}
					{toast.title}
				{/snippet}

				{#snippet content()}
					{#if toast.message}
						<Markdown content={toast.message} />
					{/if}
				{/snippet}
			</InfoMessage>
		</div>
	{/each}
</div>

<style>
	.toast-controller {
		display: flex;
		z-index: var(--z-blocker);
		position: absolute;
		right: 0;

		bottom: 0;
		flex-direction: column;
		max-width: 480px;
		max-height: 100%;
		padding: 12px 12px 12px 0;
		overflow-y: auto;
		gap: 8px;
		user-select: none;
	}
</style>
