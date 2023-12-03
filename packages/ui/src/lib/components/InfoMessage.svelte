<script lang="ts">
	export let color:
		| 'info'
		| 'info-dim'
		| 'accent'
		| 'accent-dim'
		| 'success'
		| 'error'
		| 'warning' = 'info';
	export let outline = false;
	export let clickable = false;
</script>

<div
	class="info-message"
	class:info-message_outlined={outline}
	class:info={color == 'info'}
	class:info-dim={color == 'info-dim'}
	class:accent={color == 'accent'}
	class:accent-dim={color == 'accent-dim'}
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:warning={color == 'warning'}
	on:click
	role={clickable ? 'button' : undefined}
>
	<span class="label text-base-body-11">
		<slot />
	</span>
</div>

<style lang="postcss">
	.info-message {
		display: flex;
		flex-direction: column;
		padding: var(--space-12);
		border-radius: var(--radius-m);
	}

	.info {
		background-color: var(--clr-theme-container-pale);

		& span {
			color: var(--clr-theme-scale-ntrl-30);
		}

		&.info-message_outlined {
			border: 1px solid var(--clr-theme-container-outline-light);
		}
	}

	.info-dim {
		background-color: var(--clr-theme-container-mid);

		& span {
			color: var(--clr-theme-scale-ntrl-30);
		}

		&.info-message_outlined {
			border: 1px solid var(--clr-theme-container-outline-light);
		}
	}

	.accent {
		background-color: var(--clr-theme-pop-container);

		& span {
			color: var(--clr-theme-scale-pop-20);
		}

		&.info-message_outlined {
			border: 1px solid var(--clr-theme-scale-pop-60);
		}
	}

	.accent-dim {
		background-color: var(--clr-theme-pop-container-dim);

		& span {
			color: var(--clr-theme-scale-pop-20);
		}

		&.info-message_outlined {
			border: 1px solid var(--clr-theme-scale-pop-60);
		}
	}

	.success {
		background-color: var(--clr-theme-succ-container);

		& span {
			color: var(--clr-theme-scale-succ-30);
		}

		&.info-message_outlined {
			border: 1px solid var(--clr-theme-scale-succ-60);
		}
	}

	.error {
		background-color: var(--clr-theme-err-container);

		& span {
			color: var(--clr-theme-scale-err-30);
		}

		&.info-message_outlined {
			border: 1px solid var(--clr-theme-scale-err-60);
		}
	}

	.warning {
		background-color: var(--clr-theme-warn-container);

		& span {
			color: var(--clr-theme-scale-warn-30);
		}

		&.info-message_outlined {
			border: 1px solid var(--clr-theme-scale-warn-60);
		}
	}

	.label {
		& :global(a) {
			font-weight: var(--bold-font-weight);
		}
	}
</style>
