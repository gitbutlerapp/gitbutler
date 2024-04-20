<script lang="ts">
	import InfoMessage from '$lib/components/InfoMessage.svelte';
	import { dismissToast, toastStore } from '$lib/notifications/toasts';
	import { marked } from 'marked';
	import { slide } from 'svelte/transition';

	var renderer = new marked.Renderer();
	renderer.link = function (href, title, text) {
		if (!title) title = text;
		return '<a target="_blank" href="' + href + '" title="' + href + '">' + text + '</a>';
	};
</script>

<div class="toast-controller">
	{#each $toastStore as toast (toast.id)}
		<div transition:slide={{ duration: 170 }}>
			<InfoMessage
				style={toast.style ?? 'neutral'}
				secondary="Dismiss"
				on:secondary={() => dismissToast(toast.id)}
				shadow
			>
				<svelte:fragment slot="title">
					{toast.title}
				</svelte:fragment>

				<svelte:fragment slot="content">
					{@html marked.parse(toast.message, { renderer })}
				</svelte:fragment>
			</InfoMessage>
		</div>
	{/each}
</div>

<style lang="postcss">
	.toast-controller {
		position: absolute;
		display: flex;
		flex-direction: column;
		bottom: var(--size-20);
		right: var(--size-20);
		gap: var(--size-8);
		max-width: 30rem;
		z-index: var(--z-blocker);
	}
</style>
