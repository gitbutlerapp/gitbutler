<script lang="ts">
	export let color: 'info' | 'info-dark' | 'accent' | 'success' | 'error' | 'warning';
	export let outline = false;
	export let clickable = false;
</script>

<div
	class="info-message"
	class:outline
	class:info={color == 'info'}
	class:info-dark={color == 'info-dark'}
	class:accent={color == 'accent'}
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
	}

	.info-dark {
		background-color: var(--clr-theme-container-mid);
	}

	.accent {
		background-color: var(--clr-theme-pop-container);
	}

	.success {
		background-color: var(--clr-theme-succ-container);
	}

	.error {
		background-color: var(--clr-theme-err-container);
	}

	.warning {
		background-color: var(--clr-theme-warn-container);
	}

	.outline {
		border: 1px solid red;
	}

	.label {
		& a {
			color: red;
		}
	}
</style>
