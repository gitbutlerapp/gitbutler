import type { User } from '$lib/api';
import { invoke } from '$lib/ipc';

export const get = async () => invoke<User | null>('get_user');

export const set = (params: { user: User }) => invoke<void>('set_user', params);

const del = () => invoke<void>('delete_user');
export { del as delete };
