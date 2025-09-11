import { InjectionToken } from '@gitbutler/core/context';
import type { IGitConfigService, GbConfig } from './gitConfigService';

export const CACHED_GIT_CONFIG_SERVICE = new InjectionToken<CachedGitConfigService>('CachedGitConfigService');

interface CachedValue<T> {
	value: T;
	timestamp: number;
}

/**
 * A cached wrapper around GitConfigService to avoid excessive calls to git_get_global_config.
 * This service caches frequently accessed global config values for a configurable duration.
 */
export class CachedGitConfigService implements IGitConfigService {
	private cache = new Map<string, CachedValue<any>>();
	private readonly cacheDurationMs: number;

	constructor(
		private gitConfigService: IGitConfigService,
		cacheDurationMs = 30000 // 30 seconds default cache duration
	) {
		this.cacheDurationMs = cacheDurationMs;
	}

	private isExpired(cachedValue: CachedValue<any>): boolean {
		return Date.now() - cachedValue.timestamp > this.cacheDurationMs;
	}

	private getCachedValue<T>(key: string): T | undefined {
		const cached = this.cache.get(key);
		if (cached && !this.isExpired(cached)) {
			return cached.value;
		}
		return undefined;
	}

	private setCachedValue<T>(key: string, value: T): void {
		this.cache.set(key, {
			value,
			timestamp: Date.now()
		});
	}

	/**
	 * Get a cached config value. If not in cache or expired, fetch from GitConfigService.
	 */
	async get<T extends string>(key: string): Promise<T | undefined> {
		// Check cache first
		const cachedValue = this.getCachedValue<T>(key);
		if (cachedValue !== undefined) {
			return cachedValue;
		}

		// Fetch from service and cache
		const value = await this.gitConfigService.get<T>(key);
		this.setCachedValue(key, value);
		return value;
	}

	/**
	 * Get a cached config value with default. If not in cache or expired, fetch from GitConfigService.
	 */
	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await this.get<T>(key);
		return value || defaultValue;
	}

	/**
	 * Convenience methods for frequently accessed user config
	 */
	async getUserName(): Promise<string | undefined> {
		return this.get<string>('user.name');
	}

	async getUserEmail(): Promise<string | undefined> {
		return this.get<string>('user.email');
	}

	/**
	 * Convenience method to get both user name and email in a single call
	 * This reduces the number of separate cache lookups for components that need both
	 */
	async getUserInfo(): Promise<{ name?: string; email?: string }> {
		const [name, email] = await Promise.all([
			this.getUserName(),
			this.getUserEmail()
		]);
		return { name, email };
	}

	/**
	 * Clear the cache (useful for testing or when config is known to have changed)
	 */
	clearCache(): void {
		this.cache.clear();
	}

	/**
	 * Remove a specific key from cache
	 */
	invalidateKey(key: string): void {
		this.cache.delete(key);
	}

	/**
	 * Pass-through methods that don't need caching
	 */
	async remove(key: string): Promise<undefined> {
		// Invalidate cache for this key since we're removing it
		this.invalidateKey(key);
		return this.gitConfigService.remove(key);
	}

	async set<T extends string>(key: string, value: T) {
		// Invalidate cache for this key since we're setting a new value
		this.invalidateKey(key);
		return this.gitConfigService.set(key, value);
	}

	async getGbConfig(projectId: string): Promise<GbConfig> {
		return this.gitConfigService.getGbConfig(projectId);
	}

	async setGbConfig(projectId: string, config: GbConfig): Promise<void> {
		return this.gitConfigService.setGbConfig(projectId, config);
	}

	async checkGitFetch(projectId: string, remoteName: string | null | undefined): Promise<void> {
		return this.gitConfigService.checkGitFetch(projectId, remoteName);
	}

	async checkGitPush(
		projectId: string,
		remoteName: string | null | undefined,
		branchName: string | null | undefined
	): Promise<{ name: string; ok: boolean }> {
		return this.gitConfigService.checkGitPush(projectId, remoteName, branchName);
	}
}