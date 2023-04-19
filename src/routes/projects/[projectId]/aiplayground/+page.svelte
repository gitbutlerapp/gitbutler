<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';

	const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
		let timeout: ReturnType<typeof setTimeout>;
		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => fn(...args), delay);
		};
	};

	// const chainUrl = 'http://127.0.0.1:8000';
	const chainUrl = 'https://hpkhygaffu.eu-west-1.awsapprunner.com';

	async function createSummary(text: string) {
		const response = await fetch(`${chainUrl}/summaries`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ text: text })
		});

		const data = await response.json();
		return data;
	}

	async function createChat() {
		const response = await fetch(`${chainUrl}/chats`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({})
		});

		const data = await response.json();
		return data;
	}

	async function getChatHistory(id: string) {
		const response = await fetch(`${chainUrl}/chats/${id}`, {
			method: 'GET'
		});

		const data = await response.json();
		return data;
	}

	async function newChatMessage(id: string, text: string) {
		const response = await fetch(`${chainUrl}/chats`, {
			method: 'PATCH',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ id, text: text })
		});

		const sequence = await response.json();
		return sequence;
	}

	async function addToSummary(id: string, newText: string) {
		const response = await fetch(`${chainUrl}/summaries`, {
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
		const response = await fetch(`${chainUrl}/summaries/${id}`, {
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

	let chatId = '';
	let chatHistory: string[][] = [];
	let chatInput = '';

	let ms = 2000;
	let clear: any;
	$: if (chatId) {
		clearInterval(clear);
		clear = setInterval(() => {
			getChatHistory(chatId).then((data) => {
				chatHistory = data.history;
				console.log(chatHistory);
			});
		}, ms);
	}
</script>

<div class="m-4 flex flex-col gap-4">
	<div class="flex flex-col gap-2 rounded border border-zinc-700 p-4">
		<p class="text-xl">Chat GitButler</p>
		<p>chatId: {chatId}</p>
		<Button
			role="basic"
			height="small"
			on:click={() => {
				createChat().then((data) => {
					chatId = data.id;
				});
				chatHistory = [];
				chatInput = '';
			}}>New chat</Button
		>
		<p>Chat history</p>
		<ul class="flex flex-col gap-2">
			{#each chatHistory as pair}
				{#each pair as message}
					<li class="rounded border border-zinc-700 bg-zinc-700">{message}</li>
				{/each}
			{/each}
		</ul>
		<p>New message</p>
		<input bind:value={chatInput} />
		<Button
			disabled={chatInput.length == 0 || !chatId}
			role="basic"
			height="small"
			on:click={() => {
				newChatMessage(chatId, chatInput).then((data) => {
					chatInput = '';
				});
			}}>Send</Button
		>
	</div>

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
