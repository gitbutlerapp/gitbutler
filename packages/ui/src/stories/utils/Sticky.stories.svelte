<script module lang="ts">
	import { sticky } from '$lib/utils/sticky';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Utils / Sticky Action',
		parameters: {
			docs: {
				description: {
					component:
						'A utility action that makes elements sticky with callback support for stuck state changes.'
				}
			}
		}
	});
</script>

<script lang="ts">
	let isStuck1 = $state(false);
	let isStuck2 = $state(false);
	let stickyEnabled = $state(true);
	let scrollContainer1: HTMLDivElement | undefined = $state();
	let scrollContainer2: HTMLDivElement | undefined = $state();
</script>

<Story name="Basic Sticky">
	<div
		bind:this={scrollContainer1}
		style="height: 200px; overflow-y: auto; border: 2px solid var(--clr-border-2); border-radius: 8px;"
	>
		<div
			class="sticky-header"
			class:stuck={isStuck1}
			use:sticky={{
				enabled: true,
				scrollContainer: scrollContainer1,
				onStuck: (stuck) => {
					isStuck1 = stuck;
				}
			}}
		>
			<h3>Sticky Header {isStuck1 ? '(STUCK)' : '(NOT STUCK)'}</h3>
			<p>This header will stick to the top when you scroll.</p>
		</div>

		<div style="padding: 20px;">
			<h4>Scroll Content</h4>
			<p>
				Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut
				labore et dolore magna aliqua.
			</p>
			<p>
				Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
				commodo consequat.
			</p>
			<p>
				Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla
				pariatur.
			</p>
			<p>
				Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit
				anim id est laborum.
			</p>
			<p>
				Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque
				laudantium.
			</p>
			<p>
				Totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae
				vitae dicta sunt.
			</p>
			<p>Explicabo nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit.</p>
			<p>Sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt.</p>
			<p>
				Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci
				velit.
			</p>
			<p>
				Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis suscipit laboriosam.
			</p>
		</div>
	</div>
</Story>

<Story name="Toggle Sticky">
	<div>
		<div style="margin-bottom: 16px;">
			<label>
				<input type="checkbox" bind:checked={stickyEnabled} />
				Enable Sticky
			</label>
		</div>

		<div
			style="height: 200px; overflow-y: auto; border: 2px solid var(--clr-border-2); border-radius: 8px;"
			bind:this={scrollContainer2}
		>
			<div
				class="sticky-header"
				class:stuck={isStuck2}
				class:disabled={!stickyEnabled}
				use:sticky={{
					enabled: stickyEnabled,
					scrollContainer: scrollContainer2,
					onStuck: (stuck) => {
						isStuck2 = stuck;
					}
				}}
			>
				<h3>Toggleable Sticky Header</h3>
				<p>Status: {stickyEnabled ? (isStuck2 ? 'STUCK' : 'NOT STUCK') : 'DISABLED'}</p>
			</div>

			<div style="padding: 20px;">
				<h4>Test Content</h4>
				<p>Toggle the checkbox above to enable/disable sticky behavior.</p>
				<p>When enabled, this header will stick to the top when scrolling.</p>
				<p>Lorem ipsum dolor sit amet, consectetur adipiscing elit.</p>
				<p>Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p>
				<p>Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.</p>
				<p>Duis aute irure dolor in reprehenderit in voluptate velit esse cillum.</p>
				<p>Excepteur sint occaecat cupidatat non proident, sunt in culpa.</p>
				<p>Sed ut perspiciatis unde omnis iste natus error sit voluptatem.</p>
			</div>
		</div>
	</div>
</Story>

<style>
	.sticky-header {
		margin: 0;
		padding: 12px 16px;
		border-bottom: 1px solid var(--clr-border-1);
		background-color: var(--clr-bg-1);
	}

	.sticky-header.stuck {
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
	}

	.sticky-header.disabled {
		background-color: var(--clr-bg-muted);
		opacity: 0.6;
	}

	.sticky-header h3 {
		margin: 0 0 4px 0;
		font-weight: 600;
		font-size: 16px;
	}

	.sticky-header p {
		margin: 0;
		color: var(--clr-text-2);
		font-size: 14px;
	}
</style>
