<script module lang="ts">
	import Codeblock from '$components/Codeblock.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Display / Codeblock',
		component: Codeblock,
		args: {
			label: undefined,
			content: 'const example = "Hello, World!";\nconsole.log(example);',
			marginBottom: undefined
		}
	});
</script>

<Story name="Playground">
	{#snippet template(args)}
		<Codeblock label={args.label} content={args.content} marginBottom={args.marginBottom} />
	{/snippet}
</Story>

<Story name="Basic">
	{#snippet template()}
		<Codeblock content="npm install @gitbutler/ui" />
	{/snippet}
</Story>

<Story name="With Label">
	{#snippet template()}
		<Codeblock label="bash" content="git checkout -b feature/new-branch" />
	{/snippet}
</Story>

<Story name="Multiline Code">
	{#snippet template()}
		<Codeblock
			label="typescript"
			content={`function fibonacci(n: number): number {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

console.log(fibonacci(10));`}
		/>
	{/snippet}
</Story>

<Story name="Long Code (scrollable)">
	{#snippet template()}
		<Codeblock
			label="javascript"
			content={`// This is a long piece of code that will demonstrate scrolling
const data = [
  { id: 1, name: "Alice", age: 30 },
  { id: 2, name: "Bob", age: 25 },
  { id: 3, name: "Charlie", age: 35 },
  { id: 4, name: "Diana", age: 28 },
  { id: 5, name: "Eve", age: 32 },
  { id: 6, name: "Frank", age: 45 },
  { id: 7, name: "Grace", age: 29 },
  { id: 8, name: "Henry", age: 38 },
  { id: 9, name: "Ivy", age: 27 },
  { id: 10, name: "Jack", age: 41 }
];

function processData(items) {
  return items
    .filter(item => item.age > 30)
    .map(item => ({ ...item, category: 'senior' }))
    .sort((a, b) => b.age - a.age);
}

const result = processData(data);
console.log(result);`}
		/>
	{/snippet}
</Story>

<Story name="JSON Example">
	{#snippet template()}
		<Codeblock
			label="package.json"
			content={`{
  "name": "@gitbutler/ui",
  "version": "1.0.0",
  "dependencies": {
    "svelte": "^5.0.0"
  }
}`}
		/>
	{/snippet}
</Story>

<Story name="With Custom Margin">
	{#snippet template()}
		<div>
			<Codeblock content="First block" marginBottom="24px" />
			<Codeblock content="Second block with more spacing" />
		</div>
	{/snippet}
</Story>
