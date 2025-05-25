export async function* stringStreamGenerator(
	reader: ReadableStreamDefaultReader<Uint8Array>
): AsyncGenerator<string, void, void> {
	try {
		while (true) {
			const { done, value } = await reader.read();
			if (done) {
				break;
			}
			yield new TextDecoder().decode(value);
		}
	} finally {
		reader.releaseLock();
	}
}
