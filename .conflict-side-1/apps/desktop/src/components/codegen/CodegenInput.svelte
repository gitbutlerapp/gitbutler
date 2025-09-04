<script lang="ts">
	import { Tooltip, Textarea, AsyncButton } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	type Props = {
		value: string;
		loading: boolean;
		onsubmit: () => Promise<void>;
		onAbort?: () => Promise<void>;
		onChange: (value: string) => void;
		actions: Snippet;
	};

	let { value = $bindable(), loading, onsubmit, onAbort, actions, onChange }: Props = $props();

	$effect(() => {
		onChange(value);
	});

	let showAbortButton = $state(false);

	$effect(() => {
		// Show abort button if loading for more than 1 second
		if (loading && onAbort) {
			const timer = setTimeout(() => {
				showAbortButton = true;
			}, 1000);

			return () => {
				clearTimeout(timer);
				showAbortButton = false;
			};
		} else {
			showAbortButton = false;
		}
	});

	async function handleSubmit() {
		await onsubmit();
	}

	async function handleKeypress(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();

			if (loading || value.trim().length === 0) return;

			await handleSubmit();
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="text-input dialog-input" onkeypress={handleKeypress}>
	<Textarea
		bind:value
		autofocus
		placeholder="What would you like to make..."
		borderless
		padding={{
			bottom: 60
		}}
	/>

	<div class="dialog-input__actions">
		<div class="dialog-input__actions-item">
			{@render actions()}
		</div>

		<div class="dialog-input__actions-item">
			{#if showAbortButton && onAbort}
				<div class="flex" in:fade={{ duration: 150 }} out:fade={{ duration: 100 }}>
					<AsyncButton kind="outline" style="error" action={onAbort} icon="stop" reversedDirection>
						Stop
					</AsyncButton>
				</div>
			{/if}

			<Tooltip
				text={loading ? 'Processing...' : value.trim().length === 0 ? 'Type a message' : 'Send ↵'}
			>
				<button
					class="send-button"
					type="button"
					disabled={loading || value.trim().length === 0}
					class:loading
					style="pop"
					onclick={handleSubmit}
					aria-label="Send"
				>
					<svg
						class="circle-icon"
						class:spinner={loading}
						viewBox="0 0 18 18"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<circle
							vector-effect="non-scaling-stroke"
							cx="9"
							cy="9"
							r="8.25"
							stroke="currentColor"
						/>
					</svg>

					<svg
						class="arrow-icon"
						class:spinner={loading}
						viewBox="0 0 16 16"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							vector-effect="non-scaling-stroke"
							d="M12.0195 8L8.72664 4.70711C8.33611 4.31658 7.70295 4.31658 7.31242 4.70711L4.01953 8"
							stroke="currentColor"
							stroke-width="1.5"
						/>
						<path
							d="M8.01953 4L8.01953 12"
							stroke="currentColor"
							stroke-width="1.5"
							vector-effect="non-scaling-stroke"
						/>
					</svg>
				</button>
			</Tooltip>
		</div>
	</div>
</div>

<style lang="postcss">
	.dialog-input {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 0;
	}

	.dialog-input__actions {
		display: flex;
		position: absolute;
		bottom: 0;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 12px;

		gap: 8px;
		pointer-events: none;
	}

	.dialog-input__actions-item {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 4px;
		pointer-events: all;
	}

	/* SEND BUTTON */
	.send-button {
		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: var(--size-button);
		height: var(--size-button);
		border-radius: var(--radius-btn);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
		transition:
			background-color 0.2s ease-in-out,
			transform 0.2s ease-in-out;

		&:not(:disabled):hover {
			transform: translateY(-2px);
			background-color: var(--clr-theme-pop-element-hover);

			.arrow-icon {
				transform: translate(-50%, -50%) translateY(1px);
			}
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}
	}

	.circle-icon {
		width: 18px;
		height: 18px;
		stroke-width: 1.5;
		transition: transform 0.2s;
	}

	.arrow-icon {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 16px;
		height: 16px;
		transform: translate(-50%, -50%);
		transition: transform 0.2s;
	}

	/* SPINNER ANIMATION */
	.circle-icon.spinner {
		animation: spin 1s linear infinite;
	}
	.circle-icon.spinner circle {
		--gap-length: 100;
		animation: dash 2s infinite linear;
	}
	.arrow-icon.spinner {
		opacity: 0;
		transition: opacity 0.2s ease-in-out;
	}

	@keyframes spin {
		from {
			stroke-width: 2.2;
			transform: rotate(0deg) scale(0.7);
		}
		to {
			stroke-width: 2.2;
			transform: rotate(360deg) scale(0.7);
		}
	}

	@keyframes dash {
		0% {
			stroke-dasharray: 1, var(--gap-length);
			stroke-dashoffset: 0;
		}
		50% {
			stroke-dasharray: 60, var(--gap-length);
			stroke-dashoffset: -30;
		}
		100% {
			stroke-dasharray: 100, var(--gap-length);
			stroke-dashoffset: -50;
		}
	}
</style>
