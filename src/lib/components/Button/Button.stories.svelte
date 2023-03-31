<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';

	import Button from './Button.svelte';

	const heights = ['basic', 'small'] as const;
	const widths = ['basic', 'long'] as const;
</script>

<Meta title="GitButler/Button" component={Button} />

<Story name="Basic Button">
	<div class="flex gap-6">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						<div class="flex gap-2">
							{#each [false, true] as disabled}
								<Button role="basic" {filled} {disabled} {height} {width}>Label</Button>
							{/each}
						</div>
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Primary Button">
	<div class="flex gap-6">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						<div class="flex gap-2">
							{#each [false, true] as disabled}
								<Button role="primary" {filled} {disabled} {height} {width}>Label</Button>
							{/each}
						</div>
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Destructive Button">
	<div class="flex gap-6">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						<div class="flex gap-2">
							{#each [false, true] as disabled}
								<Button role="destructive" {filled} {disabled} {height} {width}>Label</Button>
							{/each}
						</div>
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Basic Link">
	<div class="flex gap-6">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						<div class="flex gap-2">
							{#each [false, true] as disabled}
								<Button
									href="https://gitbutler.com"
									role="basic"
									{filled}
									{disabled}
									{height}
									{width}>Label</Button
								>
							{/each}
						</div>
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Primary Link">
	<div class="flex gap-6">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						<div class="flex gap-2">
							{#each [false, true] as disabled}
								<Button
									href="https://gitbutler.com"
									role="primary"
									{filled}
									{disabled}
									{height}
									{width}>Label</Button
								>
							{/each}
						</div>
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>

<Story name="Destructive Link">
	<div class="flex gap-6">
		{#each widths as width}
			{#each heights as height}
				<div class="flex flex-col gap-2">
					{#each [true, false] as filled}
						<div class="flex gap-2">
							{#each [false, true] as disabled}
								<Button
									href="https://gitbutler.com"
									role="destructive"
									{filled}
									{disabled}
									{height}
									{width}>Label</Button
								>
							{/each}
						</div>
					{/each}
				</div>
			{/each}
		{/each}
	</div>
</Story>
