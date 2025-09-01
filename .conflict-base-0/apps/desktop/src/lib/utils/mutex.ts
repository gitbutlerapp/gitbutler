export default class Mutex {
	private mutex = Promise.resolve();

	async lock<T>(fn: () => Promise<T>): Promise<T> {
		let release: () => void;
		const wait = new Promise<void>((res) => (release = res));
		const prev = this.mutex;
		this.mutex = prev.then(async () => await wait);
		await prev;
		try {
			return await fn();
		} finally {
			release!();
		}
	}
}
