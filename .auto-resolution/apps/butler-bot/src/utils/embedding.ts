import OpenAI from 'openai';

export async function createEmbedding(openai: OpenAI, text: string): Promise<number[]> {
	const response = await openai.embeddings.create({
		model: 'text-embedding-3-large',
		dimensions: 3072,
		input: text
	});

	return response.data[0]!.embedding;
}

export function stringifyEmbedding(embedding: number[]): string {
	return JSON.stringify(embedding);
}

export function parseEmbedding(embedding: string): number[] {
	return JSON.parse(embedding);
}

export function compareEmbeddings(embedding1: number[], embedding2: number[]): number {
	return cosineSimilarity(embedding1, embedding2);
}

function cosineSimilarity(a: number[], b: number[]): number {
	let dotproduct = 0;
	let mA = 0;
	let mB = 0;

	for (let i = 0; i < a.length; i++) {
		dotproduct += a[i]! * b[i]!;
		mA += a[i]! * a[i]!;
		mB += b[i]! * b[i]!;
	}

	mA = Math.sqrt(mA);
	mB = Math.sqrt(mB);
	const similarity = dotproduct / (mA * mB);

	return similarity;
}
