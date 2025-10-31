import { vi } from 'vitest';

export function mockCreateBackend() {
	const DummyBackend = vi.fn();

	DummyBackend.prototype.invoke = vi.fn();
	DummyBackend.prototype.listen = vi.fn();
	DummyBackend.prototype.platformName = 'macos';
	DummyBackend.prototype.checkUpdate = vi.fn();

	return new DummyBackend();
}
