<script lang="ts">
	import { Button, Textarea } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	type Props = {
		value: string;

		loading: boolean;

		onsubmit: () => Promise<void>;
		actions: Snippet;
	};

	let { value = $bindable(), loading, onsubmit, actions }: Props = $props();

	async function handleSubmit() {
		await onsubmit();
	}

	async function handleKeypress(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			await handleSubmit();
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="input" onkeypress={handleKeypress}>
	<Textarea
		bind:value
		placeholder="What would you like to make..."
		borderless
		padding={{
			top: 0,
			left: 0,
			right: 0,
			bottom: 0
		}}
	/>
	<div class="flex justify-between items-center">
		<div class="flex gap-4 items-center">
			{@render actions()}
		</div>
		<Button disabled={loading} {loading} style="pop" onclick={handleSubmit}>
			{#if !loading}
				<div class="svg-container">
					<svg
						xmlns="http://www.w3.org/2000/svg"
						width="16"
						height="16"
						viewBox="0 0 16 16"
						fill="none"
					>
						<g clip-path="url(#clip0_13989_3341)">
							<path
								d="M8 0C12.4182 9.89535e-05 15.9999 3.58184 16 8C16 12.4182 12.4182 15.9999 8 16C3.58172 16 0 12.4183 0 8C6.59725e-05 3.58178 3.58176 0 8 0ZM8 1.5C4.41019 1.5 1.50007 4.41021 1.5 8C1.5 11.5899 4.41015 14.5 8 14.5C11.5898 14.4999 14.5 11.5898 14.5 8C14.4999 4.41027 11.5897 1.5001 8 1.5Z"
								fill="white"
							/>
							<path
								d="M12.0195 8L8.72664 4.70711C8.33611 4.31658 7.70295 4.31658 7.31242 4.70711L4.01953 8"
								stroke="white"
								stroke-width="1.5"
							/>
							<path d="M8.01953 4L8.01953 12" stroke="white" stroke-width="1.5" />
						</g>
						<defs>
							<clipPath id="clip0_13989_3341">
								<rect width="16" height="16" fill="white" />
							</clipPath>
						</defs>
					</svg>
				</div>
			{/if}
		</Button>
	</div>
</div>

<style lang="postcss">
	.input {
		display: flex;
		flex-direction: column;
		padding: 12px;

		gap: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.svg-container {
		margin: 0 -3px;
	}
</style>
