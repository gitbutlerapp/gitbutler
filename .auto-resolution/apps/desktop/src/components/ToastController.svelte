<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { dismissToast, toastStore } from '$lib/notifications/toasts';
	import { TestId } from '$lib/testing/testIds';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { slide } from 'svelte/transition';
</script>

<div class="toast-controller hide-native-scrollbar">
	{#each $toastStore as toast (toast.id)}
		<div transition:slide={{ duration: 170 }}>
			<InfoMessage
				testId={TestId.ToastInfoMessage}
				style={toast.style ?? 'neutral'}
				secondaryLabel="Dismiss"
				error={toast.error}
				secondaryAction={() => dismissToast(toast.id)}
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
