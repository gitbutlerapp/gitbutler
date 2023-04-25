<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';
	import { IconHome } from '../icons';

	import Button from './Button.svelte';

	const heights = ['basic', 'small'] as const;
</script>

<Meta title="GitButler/Button" component={Button} />

<Story name="Basic Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each [true, false] as filled}
					{#each [true, false] as outlined}
						<div class="flex gap-2">
							{#each [false, true] as loading}
								{#each [false, true] as disabled}
									<Button
										role="basic"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										on:click={() => alert('Clicked!')}
									>
										Button
									</Button>
									<Button
										role="basic"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									>
										Button with icon
									</Button>
									<Button
										role="basic"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									/>
								{/each}
							{/each}
						</div>
					{/each}
				{/each}
			</div>
		{/each}
	</div>
</Story>

<Story name="Primary Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each [true, false] as filled}
					{#each [true, false] as outlined}
						<div class="flex gap-2">
							{#each [false, true] as loading}
								{#each [false, true] as disabled}
									<Button
										role="primary"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										on:click={() => alert('Clicked!')}
									>
										Button
									</Button>
									<Button
										role="primary"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									>
										Button with icon
									</Button>
									<Button
										role="primary"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									/>
								{/each}
							{/each}
						</div>
					{/each}
				{/each}
			</div>
		{/each}
	</div>
</Story>

<Story name="Destructive Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each [true, false] as filled}
					{#each [true, false] as outlined}
						<div class="flex gap-2">
							{#each [false, true] as loading}
								{#each [false, true] as disabled}
									<Button
										role="destructive"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										on:click={() => alert('Clicked!')}
									>
										Button
									</Button>
									<Button
										role="destructive"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									>
										Button with icon
									</Button>
									<Button
										role="destructive"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									/>
								{/each}
							{/each}
						</div>
					{/each}
				{/each}
			</div>
		{/each}
	</div>
</Story>

<Story name="Purple Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each [true, false] as filled}
					{#each [true, false] as outlined}
						<div class="flex gap-2">
							{#each [false, true] as loading}
								{#each [false, true] as disabled}
									<Button
										role="purple"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										on:click={() => alert('Clicked!')}
									>
										Button
									</Button>
									<Button
										role="purple"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									>
										Button with icon
									</Button>
									<Button
										role="purple"
										{filled}
										{disabled}
										{outlined}
										{height}
										{loading}
										icon={IconHome}
										on:click={() => alert('Clicked!')}
									/>
								{/each}
							{/each}
						</div>
					{/each}
				{/each}
			</div>
		{/each}
	</div>
</Story>
