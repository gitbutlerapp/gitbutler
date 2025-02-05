import type { LoadableData } from '$lib/network/types';

export type ApiUserSimple = {
	id: number;
	avatar_url: string | null;
	email: string | null;
	login: string | null;
	name: string | null;
};

export function isApiUserSimple(data: unknown): data is ApiUserSimple {
	return (
		typeof data === 'object' &&
		data !== null &&
		typeof (data as any).id === 'number' &&
		(typeof (data as any).avatar_url === 'string' || (data as any).avatar_url === null) &&
		(typeof (data as any).email === 'string' || (data as any).email === null) &&
		(typeof (data as any).login === 'string' || (data as any).login === null) &&
		(typeof (data as any).name === 'string' || (data as any).name === null)
	);
}

export type UserSimple = {
	id: number;
	avatarUrl: string | undefined;
	email: string | undefined;
	login: string | undefined;
	name: string | undefined;
};

export function apiToUserSimple(api: ApiUserSimple): UserSimple {
	return {
		id: api.id,
		avatarUrl: api.avatar_url ?? undefined,
		email: api.email ?? undefined,
		login: api.login ?? undefined,
		name: api.name ?? undefined
	};
}

export type ApiUserMaybe = {
	email: string;
	user: ApiUserSimple | null;
};

export type UserMaybe = {
	email: string;
	user: UserSimple | undefined;
};

export function apiToUserMaybe(api: ApiUserMaybe): UserMaybe {
	return {
		email: api.email,
		user: api.user ? apiToUserSimple(api.user) : undefined
	};
}

export type ApiUser = ApiUserSimple & {
	given_name?: string;
	family_name?: string;
	picture?: string;
	locale?: string;
	access_token?: string;
	updated_at: string;
	created_at: string;
	supporter?: boolean;
	role?: string;
};

export type User = {
	id: number;
	login?: string;
	name?: string;
	email?: string;
	avatarUrl?: string;
};

export type LoadableUser = LoadableData<User, User['id']>;

export type LoadableUserIdByLogin = LoadableData<number, string>;

export function apiToUser(apiUser: ApiUser): User {
	return {
		...apiToUserSimple(apiUser)
	};
}

export type ApiUserPrivate = ApiUser & {
	access_token: string;
	role: string | null;
};

export type UserPrivate = {
	accessToken: string;
	role: string | undefined;
};

export function apiToUserPrivate(api: ApiUserPrivate): UserPrivate {
	return {
		...apiToUser(api),
		accessToken: api.access_token,
		role: api.role ?? undefined
	};
}

export type SearchUsersApiSearchTerm = {
	value: string;
	operator?: 'EQUAL' | 'STARTS_WITH';
	case_sensitive?: boolean;
};

export type SearchUsersApiFilter = {
	field: 'login' | 'name' | 'email';
	operator: 'NULL' | 'NOT_NULL';
};

export type SearchUsersApiQuery = {
	filters?: SearchUsersApiFilter[];
	search_terms: SearchUsersApiSearchTerm[];
};

export type SearchUsersApiParams = {
	query: SearchUsersApiQuery;
};
