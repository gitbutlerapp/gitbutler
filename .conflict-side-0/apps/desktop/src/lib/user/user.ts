import { InjectionToken } from '@gitbutler/core/context';
import type { Writable } from 'svelte/store';

export const USER = new InjectionToken<Writable<User>>('User');

export type User = {
	id: number;
	name: string | undefined;
	email: string | undefined;
	picture?: string;
	locale: string | undefined;
	created_at: string;
	updated_at: string;
	access_token: string;
	role: string | undefined;
	supporter: boolean;
	login?: string;
};
