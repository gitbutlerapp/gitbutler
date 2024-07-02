<script lang="ts">
	import { dismissToast, toastStore } from '$lib/notifications/toasts';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import { marked } from 'marked';
	import { slide } from 'svelte/transition';

	var renderer = new marked.Renderer();
	renderer.link = function (href, title, text) {
		if (!title) title = text;
		return '<a target="_blank" href="' + href + '" title="' + href + '">' + text + '</a>';
	};
</script>

<div class="toast-controller hide-native-scrollbar">
	<!-- eslint-disable-next-line svelte/valid-compile -->
	{#each $toastStore as toast (toast.id)}
		<div transition:slide={{ duration: 170 }}>
			<InfoMessage
				style={toast.style ?? 'neutral'}
				secondary="Dismiss"
				error={toast.error}
				on:secondary={() => dismissToast(toast.id)}
				shadow
			>
				<svelte:fragment slot="title">
					{toast.title}
				</svelte:fragment>

				<svelte:fragment slot="content">
					{@html marked.parse(toast.message ?? '', { renderer })}
				</svelte:fragment>
			</InfoMessage>
		</div>
	{/each}
</div>

<style>
	.toast-controller {
		user-select: none;
		position: absolute;
		display: flex;
		flex-direction: column;

		bottom: 0;
		right: 0;
		padding: 12px 12px 12px 0;
		gap: 8px;
		max-width: 480px;
		z-index: var(--z-blocker);
		overflow-y: auto;
		max-height: 100%;
	}
</style>
