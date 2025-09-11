import { describe, expect, it, vi, beforeEach } from 'vitest';
import { CachedGitConfigService } from './cachedGitConfigService';
import type { IGitConfigService } from './gitConfigService';

// Mock git config service
class MockGitConfigService implements IGitConfigService {
	private callCount = 0;
	private config: Record<string, string> = {
		'user.name': 'Test User',
		'user.email': 'test@example.com'
	};

	async get<T extends string>(key: string): Promise<T | undefined> {
		this.callCount++;
		return this.config[key] as T | undefined;
	}

	getCallCount() {
		return this.callCount;
	}

	resetCallCount() {
		this.callCount = 0;
	}

	setConfigValue(key: string, value: string) {
		this.config[key] = value;
	}

	// Required interface methods (not used in tests)
	async remove(_key: string): Promise<undefined> {
		return undefined;
	}

	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await this.get<T>(key);
		return value || defaultValue;
	}

	async set<T extends string>(_key: string, _value: T): Promise<T | undefined> {
		return undefined;
	}

	async getGbConfig(_projectId: string) {
		return {} as any;
	}

	async setGbConfig(_projectId: string, _config: any): Promise<void> {
		return;
	}

	async checkGitFetch(_projectId: string, _remoteName: string | null | undefined): Promise<void> {
		return;
	}

	async checkGitPush(
		_projectId: string,
		_remoteName: string | null | undefined,
		_branchName: string | null | undefined
	): Promise<{ name: string; ok: boolean }> {
		return { name: 'push', ok: true };
	}
}

describe('CachedGitConfigService', () => {
	let mockGitConfig: MockGitConfigService;
	let cachedService: CachedGitConfigService;

	beforeEach(() => {
		mockGitConfig = new MockGitConfigService();
		cachedService = new CachedGitConfigService(mockGitConfig, 100); // 100ms cache duration for testing
	});

	it('should cache git config calls', async () => {
		// First call should hit the underlying service
		const userName1 = await cachedService.getUserName();
		expect(userName1).toBe('Test User');
		expect(mockGitConfig.getCallCount()).toBe(1);

		// Second call should use cache
		const userName2 = await cachedService.getUserName();
		expect(userName2).toBe('Test User');
		expect(mockGitConfig.getCallCount()).toBe(1); // Should still be 1
	});

	it('should cache different keys separately', async () => {
		// Fetch user name and email
		const userName = await cachedService.getUserName();
		const userEmail = await cachedService.getUserEmail();

		expect(userName).toBe('Test User');
		expect(userEmail).toBe('test@example.com');
		expect(mockGitConfig.getCallCount()).toBe(2); // One call for each key

		// Fetch again - should use cache
		await cachedService.getUserName();
		await cachedService.getUserEmail();
		expect(mockGitConfig.getCallCount()).toBe(2); // Should still be 2
	});

	it('should expire cache after duration', async () => {
		// First call
		await cachedService.getUserName();
		expect(mockGitConfig.getCallCount()).toBe(1);

		// Wait for cache to expire
		await new Promise(resolve => setTimeout(resolve, 150));

		// Second call should hit service again
		await cachedService.getUserName();
		expect(mockGitConfig.getCallCount()).toBe(2);
	});

	it('should invalidate cache when setting values', async () => {
		// Populate cache
		await cachedService.get('user.name');
		expect(mockGitConfig.getCallCount()).toBe(1);

		// Set a value (should invalidate cache)
		await cachedService.set('user.name', 'New User');

		// Next get should hit the service again
		await cachedService.get('user.name');
		expect(mockGitConfig.getCallCount()).toBe(2);
	});

	it('should provide convenient getUserInfo method', async () => {
		const userInfo = await cachedService.getUserInfo();
		expect(userInfo.name).toBe('Test User');
		expect(userInfo.email).toBe('test@example.com');
		expect(mockGitConfig.getCallCount()).toBe(2); // Both user.name and user.email fetched

		// Second call should use cache
		const userInfo2 = await cachedService.getUserInfo();
		expect(userInfo2.name).toBe('Test User');
		expect(userInfo2.email).toBe('test@example.com');
		expect(mockGitConfig.getCallCount()).toBe(2); // Should still be 2
	});

	it('should clear all cache when clearCache is called', async () => {
		// Populate cache with multiple values
		await cachedService.getUserName();
		await cachedService.getUserEmail();
		expect(mockGitConfig.getCallCount()).toBe(2);

		// Clear cache
		cachedService.clearCache();

		// Next calls should hit service again
		await cachedService.getUserName();
		await cachedService.getUserEmail();
		expect(mockGitConfig.getCallCount()).toBe(4);
	});
});