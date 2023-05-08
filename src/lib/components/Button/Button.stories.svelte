<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';
	import { IconHome } from '../icons';

	import Button from './Button.svelte';

	const heights = ['basic', 'small'] as const;
	const kinds = ['filled', 'outlined', 'plain'] as const;
</script>

<Meta title="GitButler/Button" component={Button} />

<Story name="Basic Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each kinds as kind}
					<div class="flex gap-2">
						{#each [false, true] as loading}
							{#each [false, true] as disabled}
								<Button
									color="basic"
									{disabled}
									{kind}
									{height}
									{loading}
									on:click={() => alert('Clicked!')}
								>
									Button
								</Button>
								<Button
									color="basic"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								>
									Button with icon
								</Button>
								<Button
									color="basic"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								/>
							{/each}
						{/each}
					</div>
				{/each}
			</div>
		{/each}
	</div>
</Story>

<Story name="Primary Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each kinds as kind}
					<div class="flex gap-2">
						{#each [false, true] as loading}
							{#each [false, true] as disabled}
								<Button
									color="primary"
									{disabled}
									{kind}
									{height}
									{loading}
									on:click={() => alert('Clicked!')}
								>
									Button
								</Button>
								<Button
									color="primary"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								>
									Button with icon
								</Button>
								<Button
									color="primary"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								/>
							{/each}
						{/each}
					</div>
				{/each}
			</div>
		{/each}
	</div>
</Story>

<Story name="Destructive Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each kinds as kind}
					<div class="flex gap-2">
						{#each [false, true] as loading}
							{#each [false, true] as disabled}
								<Button
									color="destructive"
									{disabled}
									{kind}
									{height}
									{loading}
									on:click={() => alert('Clicked!')}
								>
									Button
								</Button>
								<Button
									color="destructive"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								>
									Button with icon
								</Button>
								<Button
									color="destructive"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								/>
							{/each}
						{/each}
					</div>
				{/each}
			</div>
		{/each}
	</div>
</Story>

<Story name="Purple Button">
	<div class="flex flex-col gap-4">
		{#each heights as height}
			<div class="flex flex-col gap-2">
				{#each kinds as kind}
					<div class="flex gap-2">
						{#each [false, true] as loading}
							{#each [false, true] as disabled}
								<Button
									color="purple"
									{disabled}
									{kind}
									{height}
									{loading}
									on:click={() => alert('Clicked!')}
								>
									Button
								</Button>
								<Button
									color="purple"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								>
									Button with icon
								</Button>
								<Button
									color="purple"
									{disabled}
									{kind}
									{height}
									{loading}
									icon={IconHome}
									on:click={() => alert('Clicked!')}
								/>
							{/each}
						{/each}
					</div>
				{/each}
			</div>
		{/each}
	</div>
</Story>
