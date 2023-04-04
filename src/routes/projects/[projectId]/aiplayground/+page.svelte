<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';

	const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
		let timeout: ReturnType<typeof setTimeout>;
		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => fn(...args), delay);
		};
	};

	async function createSummary(text: string) {
		const response = await fetch('http://127.0.0.1:8000/summaries', {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ text: text })
		});

		const data = await response.json();
		return data;
	}

	async function addToSummary(id: string, newText: string) {
		const response = await fetch('http://127.0.0.1:8000/summaries', {
			method: 'PATCH',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ id, text: newText })
		});

		const sequence = await response.json();
		return sequence;
	}

	async function getSummary(id: string) {
		const response = await fetch(`http://127.0.0.1:8000/summaries/${id}`, {
			method: 'GET'
		});

		const data = await response.json();
		return data;
	}

	let summaryId = '';
	let input = '';
	let sequence = 0;
	let processedSeq = 0;
	let summary = '';

	$: if (summaryId && processedSeq != sequence) {
		debounce(() => {
			console.log('polling summary');
			getSummary(summaryId).then((data) => {
				summary = data.text;
				processedSeq = data.sequence;
			});
		}, 1000)();
	}
</script>

<div class="m-4 flex flex-col gap-4">
	<div class="flex flex-col gap-2">
		<p>Put things to summarize here:</p>
		<input bind:value={input} />
		<Button
			role="basic"
			disabled={input.length == 0}
			on:click={() => {
				if (!summaryId) {
					createSummary(input).then((data) => {
						summaryId = data.id;
						sequence = 1;
					});
				} else {
					addToSummary(summaryId, input).then((data) => {
						sequence = data;
					});
				}
				input = '';
			}}>Add to summary</Button
		>
	</div>
	<div class="flex flex-col gap-2">
		<p>Summary ID:</p>
		<p>{summaryId}</p>
		<p>Requested Sequence:</p>
		<p>{sequence}</p>
		<p>Processed Sequence:</p>
		<p>{processedSeq}</p>
		<p>Summary:</p>
		<p>{summary}</p>
		<Button
			role="basic"
			on:click={() => {
				summaryId = '';
				input = '';
				sequence = 0;
				processedSeq = 0;
				summary = '';
			}}>Reset</Button
		>
	</div>
</div>
